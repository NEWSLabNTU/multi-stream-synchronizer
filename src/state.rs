use crate::types::{Feedback, Key, Timestamped};
use indexmap::IndexMap;
use std::{
    cmp::Ordering::*,
    collections::VecDeque,
    ops::{
        Bound::{self, *},
        RangeBounds,
    },
    time::Duration,
};
use tokio::sync::watch;

/// The internal state maintained by [sync](crate::sync).
pub struct State<K, T>
where
    K: Key,
    T: Timestamped,
{
    /// A list of buffers indexed by K.
    pub buffers: IndexMap<K, Buffer<T>>,

    /// Marks the timestamp where messages before the point are
    /// batched and emitted.
    pub commit_ts: Option<Duration>,

    /// The maximum size of each buffer for each key.
    pub buf_size: usize,

    /// The windows size that a batch of messages should reside
    /// within.
    pub window_size: Duration,

    /// The sender where feedback messages are sent to.
    pub feedback_tx: Option<watch::Sender<Feedback<K>>>,
}

impl<K, T> State<K, T>
where
    K: Key,
    T: Timestamped,
{
    // pub fn print_debug_info(&self) {
    //     debug!("buffer sizes");
    //     self.buffers.iter().for_each(|(device, buffer)| {
    //         debug!("- {}:\t{}", device, buffer.buffer.len());
    //     });
    // }

    /// Generate a feedback message.
    pub fn update_feedback(&mut self) {
        let Some(feedback_tx) = &self.feedback_tx else {
            return;
        };

        let accepted_keys: Vec<K> = self
            .buffers
            .iter()
            .filter(|&(_key, buffer)| (buffer.buffer.len() < self.buf_size))
            .map(|(key, _buffer)| *key)
            .collect();

        // Request input sources to deliver messages with ts below thresh_ts
        // let thresh_ts = self
        //     .buffers
        //     .values()
        //     .filter_map(|buffer| buffer.last_ts())
        //     .min();
        // let include_thresh_ts = self.buffers.values().all(|buffer| buffer.buffer.is_empty());

        let msg = Feedback {
            accepted_keys,
            // accepted_max_timestamp: thresh_ts.map(|ts| ts.as_nanos() as u64),
            // inclusive: Some(include_thresh_ts),
            accepted_max_timestamp: None,
            commit_timestamp: self.commit_ts,
        };

        // if self.verbose_debug {
        //     debug!("update feedback with accepted devices:");
        //     accepted_devices.iter().for_each(|device| {
        //         debug!("- {:?}", device);
        //     });
        // }

        if feedback_tx.send(msg).is_err() {
            self.feedback_tx = None;
        }
    }

    /// Try to group up messages within a time window.
    pub fn try_match(&mut self) -> Option<IndexMap<K, T>> {
        type DurationBound = (Bound<Duration>, Bound<Duration>);

        // make sure (sup - inf >= window_size)
        let (inf, _sup) = match (self.inf_timestamp(), self.sup_timestamp()) {
            (Some(inf), Some(sup)) if inf + self.window_size <= sup => (inf, sup),
            _ => return None,
        };
        let window_start = inf.saturating_sub(self.window_size);
        let window_end = inf.saturating_add(self.window_size);

        // drop_range is the range below start of window and commit timestamp
        let drop_range: DurationBound = {
            let upper = match self.commit_ts {
                Some(commit_ts) if commit_ts > window_start => Included(commit_ts),
                _ => Excluded(window_start),
            };
            (Unbounded, upper)
        };

        // untouched range is the range above the end of window timestamp
        let untouched_range: DurationBound = (Excluded(window_end), Unbounded);

        let items: IndexMap<_, _> = self
            .buffers
            .iter_mut()
            .flat_map(|(key, buffer)| -> Option<_> {
                // find the first candidate that is within the window
                let mut candidate = loop {
                    let front = buffer.buffer.pop_front()?;
                    let curr_ts = front.timestamp();

                    if drop_range.contains(&curr_ts) {
                        continue;
                    } else if untouched_range.contains(&curr_ts) {
                        return None;
                    } else {
                        break front;
                    }
                };

                // find a better candidate with minimum time difference to inf timestamp
                let mut curr_diff = duration_diff(inf, candidate.timestamp());

                loop {
                    let front = buffer.buffer.front()?;
                    let new_ts = front.timestamp();

                    if untouched_range.contains(&new_ts) {
                        break;
                    }
                    let new_diff = duration_diff(inf, new_ts);

                    if curr_diff > new_diff {
                        candidate = buffer.buffer.pop_front().unwrap();
                        curr_diff = new_diff;
                    } else {
                        break;
                    }
                }

                Some((*key, candidate))
            })
            .collect();

        // update commit timestamp
        let new_commit_ts = items.values().map(|item| item.timestamp()).min().unwrap();
        self.commit_ts = Some(new_commit_ts);

        Some(items)
    }

    /// Gets the minimum of the maximum timestamps from each buffer.
    pub fn sup_timestamp(&self) -> Option<Duration> {
        self.buffers
            .values()
            .map(|buffer| buffer.buffer.back().map(|item| item.timestamp()))
            .min_by(|lhs, rhs| match (lhs, rhs) {
                (Some(lhs), Some(rhs)) => lhs.cmp(rhs),
                (Some(_), None) => Greater,
                (None, Some(_)) => Less,
                (None, None) => Equal,
            })
            .flatten()
    }

    /// Gets the maximum of the minimum timestamps from each buffer.
    pub fn inf_timestamp(&self) -> Option<Duration> {
        self.buffers
            .values()
            .map(|buffer| buffer.buffer.front().map(|item| item.timestamp()))
            .min_by(|lhs, rhs| match (lhs, rhs) {
                (Some(lhs), Some(rhs)) => lhs.cmp(rhs).reverse(),
                (Some(_), None) => Greater,
                (None, Some(_)) => Less,
                (None, None) => Equal,
            })
            .flatten()
    }

    /// Gets the minimum timestamp among all messages.
    pub fn min_timestamp(&self) -> Option<Duration> {
        self.buffers
            .values()
            .map(|buffer| buffer.buffer.front().map(|item| item.timestamp()))
            .min_by(|lhs, rhs| match (lhs, rhs) {
                (Some(lhs), Some(rhs)) => lhs.cmp(rhs),
                (Some(_), None) => Greater,
                (None, Some(_)) => Less,
                (None, None) => Equal,
            })
            .flatten()
    }

    /// Checks if every buffer size reaches the limit.
    pub fn is_full(&self) -> bool {
        self.buffers
            .values()
            .all(|buffer| buffer.buffer.len() >= self.buf_size)
    }

    /// Checks if every buffer receives at least two messages.
    pub fn is_ready(&self) -> bool {
        self.buffers.values().all(|buffer| buffer.buffer.len() >= 2)
    }

    /// Checks if all buffers are empty.
    pub fn is_empty(&self) -> bool {
        self.buffers.values().all(|buffer| buffer.buffer.is_empty())
    }

    /// Remove the message with the minimum timestamp among all
    /// buffers. Returns true if a message is dropped.
    pub fn drop_min(&mut self) -> bool {
        let Some(min_timestamp) = self.min_timestamp() else {
            return false;
        };

        self.buffers.values_mut().for_each(|buffer| {
            if let Some(front) = buffer.buffer.front() {
                if front.timestamp() == min_timestamp {
                    buffer.buffer.pop_front();
                }
            }
        });

        true
    }

    /// Insert a message to the queue identified by the key. It
    /// returns true if the message is successfully inserted.
    pub fn push(&mut self, key: K, item: T) -> bool {
        let timestamp = item.timestamp();

        match self.commit_ts {
            Some(commit_ts) if commit_ts >= timestamp => return false,
            _ => {}
        }

        let Some(buffer) = self.buffers.get_mut(&key) else {
            return false;
        };

        buffer.try_push(item)
    }
}

pub struct Buffer<T>
where
    T: Timestamped,
{
    pub buffer: VecDeque<T>,
    pub last_ts: Option<Duration>,
}

impl<T> Buffer<T>
where
    T: Timestamped,
{
    // pub fn front_ts(&self) -> Option<Duration> {
    //     self.buffer
    //         .front()
    //         .map(|item| item.timestamp())
    //         .or(self.last_ts)
    // }

    // pub fn last_ts(&self) -> Option<Duration> {
    //     self.last_ts
    // }

    /// Try to push a message into the buffer.
    ///
    /// If the timestamp on the message is below that of the
    /// previously inserted message, the message is dropped and the
    /// method returns false. Otherwise, it stores and message and
    /// returns true.
    pub fn try_push(&mut self, item: T) -> bool {
        let timestamp = item.timestamp();

        match self.last_ts {
            Some(last_ts) if last_ts >= timestamp => return false,
            _ => {}
        }

        self.last_ts = Some(timestamp);
        self.buffer.push_back(item);
        true
    }
}

fn duration_diff(lhs: Duration, rhs: Duration) -> Duration {
    if lhs >= rhs {
        lhs - rhs
    } else {
        rhs - lhs
    }
}

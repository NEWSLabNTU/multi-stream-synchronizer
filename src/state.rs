use crate::{
    buffer::Buffer,
    types::{Feedback, Key, WithTimestamp},
};
use indexmap::IndexMap;
use std::time::Duration;
use tokio::sync::watch;

/// The internal state maintained by [sync](crate::sync).
#[derive (Debug)]
pub struct State<K, T>
where
    K: Key,
    T: WithTimestamp,
{
    /// A list of buffers indexed by key K.
    pub buffers: IndexMap<K, Buffer<T>>,

    /// Marks the timestamp where messages before the time point are
    /// emitted.
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
    T: WithTimestamp,
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
            .filter(|&(_key, buffer)| (buffer.len() < self.buf_size))
            .map(|(key, _buffer)| key.clone())
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
        let inf_ts = loop {
            let (_, inf_ts) = self.inf_timestamp()?;

            // Checking all buffers have only one data left.
            // Make sure (sup - inf >= window_size). If not, it needs to
            // wait for more messages.
            let (_, sup_ts) = self.sup_timestamp()?;
            if !self.all_one(){
                if inf_ts + self.window_size > sup_ts {
                    return None;
                }
            }
            

            let window_start = inf_ts.saturating_sub(self.window_size);

            // Drop messages before the time window.
            let dropped = self.buffers.values_mut().any(|buffer| {
                let count = buffer.drop_before(window_start);
                count > 0
            });

            if !dropped {
                break inf_ts;
            }
        };

        // let window_start = inf_ts.saturating_sub(self.window_size);
        let window_end = inf_ts.saturating_add(self.window_size);

        let items: IndexMap<_, _> = self
            .buffers
            .iter_mut()
            .map(|(key, buffer)| {
                // find the first candidate that is within the window
                let item = buffer.pop_front().unwrap();
                assert!(item.timestamp() <= window_end);
                (key.clone(), item)
            })
            .collect();

        // update commit timestamp
        let new_commit_ts = items.values().map(|item| item.timestamp()).min().unwrap();
        self.commit_ts = Some(new_commit_ts);

        Some(items)
    }

    /// Gets the minimum of the maximum timestamps from each buffer.
    pub fn sup_timestamp(&self) -> Option<(K, Duration)> {
        self.buffers
            .iter()
            .filter_map(|(key, buffer)| {
                // Get the latest timestamp
                let ts = buffer.back()?.timestamp();
                Some((key.clone(), ts))
            })
            .min_by_key(|(_, ts)| *ts)
    }

    /// Gets the maximum of the minimum timestamps from each buffer.
    pub fn inf_timestamp(&self) -> Option<(K, Duration)> {
        self.buffers
            .iter()
            .filter_map(|(key, buffer)| {
                // Get the earliest timestamp
                let ts = buffer.front()?.timestamp();
                Some((key.clone(), ts))
            })
            .max_by_key(|(_, ts)| *ts)
    }

    /// Gets the minimum timestamp among all messages.
    pub fn min_timestamp(&self) -> Option<(K, Duration)> {
        self.buffers
            .iter()
            .filter_map(|(key, buffer)| {
                // Get the earliest timestamp
                let ts = buffer.front()?.timestamp();
                Some((key.clone(), ts))
            })
            .min_by_key(|(_, ts)| *ts)
    }

    /// Checks if every buffer size reaches the limit.
    pub fn is_full(&self) -> bool {
        self.buffers
            .values()
            .all(|buffer| buffer.len() >= self.buf_size)
    }

    /// Checks if every buffer receives at least two messages.
    pub fn is_ready(&self) -> bool {
        self.buffers.values().all(|buffer| buffer.len() >= 2)
    }

    /// Checks if there are buffers which are empty.
    pub fn is_empty(&self) -> bool {
        // self.buffers.values().all(|buffer| buffer.is_empty())
        let buffers = self.buffers.iter();
        for item in buffers{
            let (_key, buffer) = item;
            if buffer.is_empty(){
                return true;
            } else{
                continue;
            }
        }
        false
        
    }

    /// Checks if all buffers have only one data left.
    pub fn all_one(&self) -> bool {
        self.buffers.values().all(|buffer| buffer.len() == 1)
    }
    /// Remove the message with the minimum timestamp among all
    /// buffers. Returns true if a message is dropped.
    pub fn drop_min(&mut self) -> bool {
        let Some((_, min_ts)) = self.min_timestamp() else {
            return false;
        };

        self.buffers.values_mut().for_each(|buffer| {
            if let Some(front) = buffer.front() {
                if front.timestamp() == min_ts {
                    buffer.pop_front();
                }
            }
        });

        true
    }

    /// Insert a message to the queue identified by the key. It
    /// returns true if the message is successfully inserted.
    pub fn push(&mut self, key: K, item: T) -> Result<(), T> {
        let timestamp = item.timestamp();

        match self.commit_ts {
            Some(commit_ts) if commit_ts >= timestamp => return Err(item),
            _ => {}
        }

        let Some(buffer) = self.buffers.get_mut(&key) else {
            return Err(item);
        };

        buffer.try_push(item)
    }
}

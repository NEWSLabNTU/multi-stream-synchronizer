use crate::types::WithTimestamp;
use std::{collections::VecDeque, time::Duration};

/// A buffer to store a sequence of messages with monotonically
/// increasing timestamps.
pub struct Buffer<T>
where
    T: WithTimestamp,
{
    buffer: VecDeque<T>,
    last_ts: Option<Duration>,
}

impl<T> Buffer<T>
where
    T: WithTimestamp,
{
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            last_ts: None,
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn front(&self) -> Option<&T> {
        self.buffer.front()
    }

    pub fn back(&self) -> Option<&T> {
        self.buffer.back()
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.buffer.pop_front()
    }

    pub fn front_entry(&mut self) -> Option<FrontEntry<'_, T>> {
        let item = self.buffer.pop_front()?;
        Some(FrontEntry {
            buffer: self,
            item: Some(item),
        })
    }

    // pub fn back_entry(&mut self) -> Option<BackEntry<'_, T>> {
    //     let item = self.buffer.pop_back()?;
    //     Some(BackEntry {
    //         buffer: self,
    //         item: Some(item),
    //     })
    // }

    // pub fn pop_back(&mut self) -> Option<T> {
    //     self.buffer.pop_back()
    // }

    // pub fn front_ts(&self) -> Option<Duration> {
    //     self.buffer
    //         .front()
    //         .map(|item| item.timestamp())
    //         .or(self.last_ts)
    // }

    // pub fn last_ts(&self) -> Option<Duration> {
    //     self.last_ts
    // }

    /// Drops messages before the a specific timestamp and returns the
    /// number of dropped messages.
    pub fn drop_before(&mut self, ts: Duration) -> usize {
        let mut count = 0;

        loop {
            let Some(entry) = self.front_entry() else {
                break;
            };

            if entry.value().timestamp() >= ts {
                break;
            } else {
                let _ = entry.take();
                count += 1;
            }
        }

        count
    }

    /// Try to push a message into the buffer.
    ///
    /// If the timestamp on the message is below that of the
    /// previously inserted message, the message is dropped and the
    /// method returns false. Otherwise, it stores and message and
    /// returns true.
    pub fn try_push(&mut self, item: T) -> Result<(), T> {
        let timestamp = item.timestamp();

        // Ensure that the inserted message has greater timestamp than
        // the latest timestamp.
        match self.last_ts {
            Some(last_ts) if last_ts >= timestamp => return Err(item),
            _ => {}
        }

        self.last_ts = Some(timestamp);
        self.buffer.push_back(item);
        Ok(())
    }
}

pub struct FrontEntry<'a, T>
where
    T: WithTimestamp,
{
    buffer: &'a mut Buffer<T>,
    item: Option<T>,
}

impl<'a, T> FrontEntry<'a, T>
where
    T: WithTimestamp,
{
    pub fn take(mut self) -> T {
        self.item.take().unwrap()
    }

    pub fn value(&self) -> &T {
        self.item.as_ref().unwrap()
    }
}

impl<'a, T> Drop for FrontEntry<'a, T>
where
    T: WithTimestamp,
{
    fn drop(&mut self) {
        if let Some(item) = self.item.take() {
            self.buffer.buffer.push_front(item);
        }
    }
}

// pub struct BackEntry<'a, T>
// where
//     T: WithTimestamp,
// {
//     buffer: &'a mut Buffer<T>,
//     item: Option<T>,
// }

// impl<'a, T> BackEntry<'a, T>
// where
//     T: WithTimestamp,
// {
//     pub fn take(mut self) -> T {
//         self.item.take().unwrap()
//     }

//     pub fn value(&self) -> &T {
//         self.item.as_ref().unwrap()
//     }
// }

// impl<'a, T> Drop for BackEntry<'a, T>
// where
//     T: WithTimestamp,
// {
//     fn drop(&mut self) {
//         if let Some(item) = self.item.take() {
//             self.buffer.buffer.push_back(item);
//         }
//     }
// }

use super::Timestamped;
use std::{collections::VecDeque, time::Duration};

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

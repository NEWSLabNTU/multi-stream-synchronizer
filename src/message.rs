use crate::sync::Timestamped;
use std::{ops::Bound, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DevicePath(pub usize);

/// Feedback message from matcher to pcd and video sources.
#[derive(Debug, Clone)]
pub struct MatcherFeedback {
    pub accepted_max_timestamp: Option<Duration>,
    pub commit_timestamp: Option<Duration>,
    pub inclusive: Option<bool>,
    pub accepted_devices: Vec<DevicePath>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataFrame {
    /// The frame index given by message-matcher.
    pub frame_id: u64,
    /// the minimal timestamp among all devices computed after message-matcher
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputMessage {
    pub timestamp: Duration,
}

impl Timestamped for InputMessage {
    fn timestamp(&self) -> Duration {
        self.timestamp
    }
}

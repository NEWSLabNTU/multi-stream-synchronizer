use std::time::Duration;

/// Configuration parameters that are passed to [sync](crate::sync());
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub window_size: Duration,
    pub start_time: Option<Duration>,
    pub buf_size: usize,
}

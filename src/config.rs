use std::time::Duration;

/// Configuration parameters that are passed to [sync](crate::sync());
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The time span that the grouped frames must fit within.
    pub window_size: Duration,
    /// Accepted minimum timestamps for input frames.
    pub start_time: Option<Duration>,
    /// The maximum number of frames kept for each input stream.
    pub buf_size: usize,
}

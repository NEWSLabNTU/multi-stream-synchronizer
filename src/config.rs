use chrono::NaiveDateTime;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    // pub show_verbose_debug: Option<bool>,
    pub params: MatchingParams,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchingParams {
    pub complete_matching_only: bool,
    #[serde(with = "humantime_serde")]
    pub window_size: Duration,
    pub start_time: Option<NaiveDateTime>,
    pub max_pending_msgs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInput {
    pub pcd: IndexSet<String>,
    pub video: IndexSet<String>,
}

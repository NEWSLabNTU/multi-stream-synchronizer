use anyhow::Result;
use futures::stream::BoxStream;
use indexmap::IndexMap;
use std::{hash::Hash, time::Duration};
use tokio::sync::watch;

/// Creates a timestamp from the message passed to the synchronizer.
pub trait Timestamped: Send {
    fn timestamp(&self) -> Duration;
}

/// The key that identifies the queue in the synchronizer.
pub trait Key: Clone + Copy + PartialEq + Eq + Hash + Sync + Send {}

impl<K> Key for K where K: Clone + Copy + PartialEq + Eq + Hash + Sync + Send {}

/// The feedback message generated from [sync](crate::sync()) to control
/// the pace of input streams.
#[derive(Debug, Clone)]
pub struct Feedback<K>
where
    K: Key,
{
    pub accepted_max_timestamp: Option<Duration>,
    pub commit_timestamp: Option<Duration>,
    pub accepted_keys: Vec<K>,
}

/// The stream is returned by [sync](crate::sync()), emitting batches of
/// messages within a time window.
pub type OutputStream<'a, K, T> = BoxStream<'a, Result<IndexMap<K, T>>>;

/// The stream is returned by [sync](crate::sync()) to control the pace
/// of input stream.
pub type FeedbackReceiver<K> = watch::Receiver<Feedback<K>>;

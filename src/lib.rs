//! This library group up messages close in time within a time window
//! from multiple message streams, each identified by a key.
//!
//! # Usage
//!
//! ```rust
//! use futures::{
//!     stream,
//!     stream::{StreamExt, TryStreamExt},
//! };
//! use indexmap::IndexMap;
//! use multi_stream_synchronizer::{sync, Config, Timestamped};
//! use std::time::Duration;
//!
//! // Define your message type
//! struct MyMessage(Duration);
//!
//! impl Timestamped for MyMessage {
//!     fn timestamp(&self) -> Duration {
//!         self.0
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // Create two message streams
//! let stream_x = stream::iter([
//!     MyMessage(Duration::from_millis(1001)),
//!     MyMessage(Duration::from_millis(1999)),
//!     MyMessage(Duration::from_millis(3000)),
//! ]);
//! let stream_y = stream::iter([
//!     MyMessage(Duration::from_millis(998)),
//!     MyMessage(Duration::from_millis(2003)),
//!     MyMessage(Duration::from_millis(3002)),
//! ]);
//!
//! // Join two streams into one, where each message is identified by
//! // key.
//! let join_stream = stream::select(
//!     stream_x.map(|msg| ("X", msg)),
//!     stream_y.map(|msg| ("Y", msg)),
//! )
//! .map(|msg| anyhow::Ok(msg));
//!
//! // Run the synchronization algorithm
//! let config = Config {
//!     window_size: Duration::from_millis(500),
//!     start_time: None,
//!     buf_size: 16,
//! };
//! let (sync_stream, feedback_stream) = sync(join_stream, ["X", "Y"], config)?;
//!
//! // Collect the groups
//! let groups: Vec<IndexMap<&str, MyMessage>> = sync_stream.try_collect().await?;
//! # Ok(())
//! # }
//! ```

mod buffer;
mod config;
mod state;
mod sync;
mod types;
mod utils;

pub use config::Config;
pub use sync::sync;
pub use types::*;

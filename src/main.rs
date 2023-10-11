mod common;
mod config;
mod message;
mod sync;
mod utils;

use crate::{message as msg, msg::DataFrame};
use anyhow::Result;
use futures::{
    self,
    stream::{self, StreamExt as _, TryStreamExt as _},
};
use indexmap::IndexSet;
use msg::{DevicePath, InputMessage};
use std::{path::PathBuf, sync::Arc, time::Duration};
use structopt::StructOpt;
use tokio_stream::wrappers::WatchStream;

#[derive(Debug, Clone, StructOpt)]
/// LiDAR data capturing and point cloud producing service.
struct Args {
    /// config path
    #[structopt(long)]
    pub config: PathBuf,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    pretty_env_logger::init();

    // parse arguments
    let Args {
        config: config_path,
    } = Args::from_args();

    // load config
    let config: config::Config = todo!();
    let config = Arc::new(config);

    // enumerate valid devices
    let valid_devices: IndexSet<DevicePath> = todo!();

    // create image/pcd input stream
    let input_stream = {
        // TODO
        stream::repeat_with(|| {
            anyhow::Ok((
                DevicePath(0),
                InputMessage {
                    timestamp: Duration::ZERO,
                },
            ))
        })
    };

    let (output_stream, feedback_rx) = {
        let config::MatchingParams {
            complete_matching_only,
            window_size,
            start_time,
            max_pending_msgs,
            ..
        } = config.params;
        let start_time = start_time.map(|ts| Duration::from_nanos(ts.timestamp_nanos() as u64));
        let buf_size = max_pending_msgs;

        sync::sync(
            input_stream.boxed(),
            complete_matching_only,
            window_size,
            start_time,
            buf_size,
            valid_devices,
        )?
    };

    let output_stream = output_stream
        .enumerate()
        .map(|(idx, result)| anyhow::Ok((idx, result?)))
        .map_ok(|(frame_id, matching)| {
            // classify messages
            let min_timestamp = u64::MAX;

            matching.into_iter().for_each(|(_device, _item)| {});

            let min_timestamp = if min_timestamp == u64::MAX {
                panic!("min_timestamp is None");
            } else {
                Some(min_timestamp)
            };

            DataFrame {
                frame_id: frame_id as u64,
                timestamp: min_timestamp,
            }
        });

    let producer_future = output_stream.for_each(|_| async move {});
    let feedback_future = WatchStream::new(feedback_rx).for_each(|_| async move {});

    // wait for all workers to finish
    futures::join!(producer_future, feedback_future);

    Ok(())
}

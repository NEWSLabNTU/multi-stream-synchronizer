mod buffer;
mod state;

use self::{buffer::Buffer, state::State};
use crate::msg::{DevicePath, MatcherFeedback};
use anyhow::{ensure, Result};
use futures::{
    self,
    stream::{Stream, TryStreamExt as _},
};
use indexmap::{IndexMap, IndexSet};
use log::{debug, warn};
use pin_project::pin_project;
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll, Poll::*},
    time::Duration,
};
use tokio::sync::watch;

pub trait Timestamped {
    fn timestamp(&self) -> Duration;
}

pub fn sync<S, T>(
    input_stream: S,
    complete_matching: bool,
    window_size: Duration,
    start_time: Option<Duration>,
    buf_size: usize,
    devices: IndexSet<DevicePath>,
) -> Result<(
    impl Stream<Item = Result<IndexMap<DevicePath, T>>> + Send,
    watch::Receiver<MatcherFeedback>,
)>
where
    S: 'static + Stream<Item = Result<(DevicePath, T)>> + Unpin + Send,
    T: 'static + Timestamped + Send,
{
    let num_devices = devices.len();

    let config = SynchronizerConfig {
        window_size,
        start_time,
        buf_size,
        devices,
    };
    let (stream, feedback_rx) = Synchronizer::new(input_stream, config)?;
    let stream = stream.try_filter(move |matching| {
        let ok = !complete_matching || matching.len() == num_devices;
        if !ok {
            debug!("drop a matching due to incomplete matching");
        }
        async move { ok }
    });

    Ok((stream, feedback_rx))
}

#[derive(Debug, Clone)]
struct SynchronizerConfig {
    pub window_size: Duration,
    pub start_time: Option<Duration>,
    pub buf_size: usize,
    pub devices: IndexSet<DevicePath>,
}

#[pin_project]
struct Synchronizer<S, T>
where
    S: Stream<Item = Result<(DevicePath, T)>>,
    T: Timestamped,
{
    #[pin]
    stream: Option<S>,
    state: State<T>,
}

impl<S, T> Synchronizer<S, T>
where
    S: Stream<Item = Result<(DevicePath, T)>>,
    T: Timestamped,
{
    pub fn new(
        stream: S,
        config: SynchronizerConfig,
    ) -> Result<(Self, watch::Receiver<MatcherFeedback>)> {
        let SynchronizerConfig {
            window_size,
            start_time,
            buf_size,
            devices,
        } = config;

        ensure!(buf_size >= 2);
        ensure!(window_size > Duration::ZERO);
        ensure!(!devices.is_empty());

        let buffers = devices
            .iter()
            .cloned()
            .map(|device| {
                let buffer = Buffer {
                    buffer: VecDeque::with_capacity(buf_size),
                    last_ts: None,
                };

                (device, buffer)
            })
            .collect();
        let (feedback_tx, feedback_rx) = watch::channel(MatcherFeedback {
            accepted_max_timestamp: None,
            commit_timestamp: None,
            inclusive: None,
            accepted_devices: devices.iter().cloned().collect(),
        });

        let state = State {
            feedback_tx: Some(feedback_tx),
            buffers,
            commit_ts: start_time,
            buf_size,
            window_size,
        };

        let me = Self {
            stream: Some(stream),
            state,
        };

        Ok((me, feedback_rx))
    }
}

impl<S, T> Stream for Synchronizer<S, T>
where
    S: Stream<Item = Result<(DevicePath, T)>>,
    T: Timestamped,
{
    type Item = Result<IndexMap<DevicePath, T>>;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut me = self.project();
        let state = &mut me.state;

        Ready(if let Some(mut stream) = me.stream.as_mut().as_pin_mut() {
            loop {
                if !state.is_ready() {
                    // the any is device
                    let item = stream.as_mut().poll_next(ctx);

                    let (device, item) = match item {
                        Ready(Some(Ok(item))) => item,
                        Ready(Some(Err(err))) => {
                            me.stream.set(None);
                            break Some(Err(err));
                        }
                        Ready(None) => {
                            me.stream.set(None);
                            break None;
                        }
                        Pending => {
                            return Pending;
                        }
                    };

                    if !state.push(&device, item) {
                        debug!("drop a late message for device {:?}", device);
                        state.update_feedback(); // tell upstream to catch up
                        continue;
                    }
                    state.update_feedback();
                } else if state.is_full() {
                    if let Some(matching) = state.try_match() {
                        state.update_feedback();
                        break Some(Ok(matching));
                    } else {
                        warn!(
                            "Unable to find a new matching while all buffers are full.\
                             Drop one message anyway."
                        );
                        state.drop_min();
                        state.update_feedback();
                    }
                } else {
                    let item = stream.as_mut().poll_next(ctx);

                    let (device, item) = match item {
                        Ready(Some(Ok(item))) => item,
                        Ready(Some(Err(err))) => {
                            me.stream.set(None);
                            break Some(Err(err));
                        }
                        Ready(None) => {
                            me.stream.set(None);
                            break None;
                        }
                        Pending => {
                            return Pending;
                        }
                    };

                    if !state.push(&device, item) {
                        debug!("drop a late message for device {:?}", device);
                        state.update_feedback(); // tell upstream to catch up
                        continue;
                    }

                    if let Some(matching) = state.try_match() {
                        state.update_feedback();
                        break Some(Ok(matching));
                    } else {
                        state.update_feedback();
                    }
                }
            }
        } else {
            loop {
                if state.is_empty() {
                    break None;
                } else if let Some(matching) = state.try_match() {
                    break Some(Ok(matching));
                } else {
                    state.drop_min();
                }
            }
        })
    }
}

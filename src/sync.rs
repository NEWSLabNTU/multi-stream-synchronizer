use crate::{
    state::{Buffer, State},
    types::{FeedbackReceiver, Key, OutputStream, Timestamped},
    Config, Feedback,
};
use anyhow::{ensure, Result};
use futures::{
    self,
    stream::{self, Stream},
    StreamExt,
};
use indexmap::IndexMap;
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll, Poll::*},
    time::Duration,
};
use tokio::sync::watch;
use tracing::warn;

/// Consume a stream of messages, each identified by a key, and group
/// up messages within a time window with distinct keys.
///
/// The function returns an output stream and a feedback stream. The
/// output stream emits batches of grouped messages. The feedback
/// stream emits feedback messages to control the input stream.
pub fn sync<'a, K, T, S, I>(
    stream: S,
    keys: I,
    config: Config,
) -> Result<(OutputStream<'a, K, T>, FeedbackReceiver<K>)>
where
    K: Key + 'a,
    T: Timestamped + 'a,
    S: Stream<Item = Result<(K, T)>> + Unpin + Send + 'a,
    I: IntoIterator<Item = K>,
{
    // let keys: Vec<_> = keys.into_iter().collect();

    let Config {
        window_size,
        start_time,
        buf_size,
    } = config;

    // Sanity check
    ensure!(buf_size >= 2);
    ensure!(window_size > Duration::ZERO);

    // Initialize buffers for respective keys.
    let buffers: IndexMap<_, _> = keys
        .into_iter()
        .map(|key| {
            let buffer = Buffer {
                buffer: VecDeque::with_capacity(buf_size),
                last_ts: None,
            };

            (key, buffer)
        })
        .collect();
    ensure!(!buffers.is_empty());

    // Create the queue that pipes generated feedback messages.
    let (feedback_tx, feedback_rx) = {
        let init_feedback = Feedback {
            accepted_max_timestamp: None,
            commit_timestamp: None,
            accepted_keys: buffers.keys().cloned().collect(),
        };
        watch::channel(init_feedback)
    };

    // Initialize the internal state.
    let mut state = State {
        feedback_tx: Some(feedback_tx),
        buffers,
        commit_ts: start_time,
        buf_size,
        window_size,
    };

    // Construct output stream.
    let output_stream = {
        let mut stream = Some(stream);
        stream::poll_fn(move |ctx| poll(Pin::new(&mut stream), &mut state, ctx))
    };

    Ok((output_stream.boxed(), feedback_rx))
}

/// The polling function is repeated called to generated batched
/// messages.
fn poll<K, T, S>(
    mut input_stream: Pin<&mut Option<S>>,
    state: &mut State<K, T>,
    ctx: &mut Context<'_>,
) -> Poll<Option<Result<IndexMap<K, T>>>>
where
    K: Key,
    S: Stream<Item = Result<(K, T)>> + Unpin + Send,
    T: Timestamped + Send,
{
    let group = if let Some(mut input_stream_mut) = input_stream.as_mut().as_pin_mut() {
        // Case: the input stream is not depleted yet.

        // Loop until a valid group is found.
        loop {
            if !state.is_ready() {
                // Case: Any one of the buffer has one or zero
                // message.

                // Consume one message from the input stream.
                let item = input_stream_mut.as_mut().poll_next(ctx);

                let (key, item) = match item {
                    Ready(Some(Ok(item))) => item, // A message is returned
                    Ready(Some(Err(err))) => {
                        // An error is returned
                        input_stream.set(None);
                        break Some(Err(err));
                    }
                    Ready(None) => {
                        // The input stream is depleted.
                        input_stream.set(None);
                        break None;
                    }
                    Pending => {
                        // The input stream is not ready.
                        return Pending;
                    }
                };

                // Try to insert the message.
                let yes = state.push(key, item);
                state.update_feedback();

                // If failed, tell the input stream to catch up and
                // retry.
                if !yes {
                    // debug!("drop a late message for device {:?}", device);
                    continue;
                }
            } else if state.is_full() {
                // Case: All buffers are full.

                // Try to group up messages. If successful, return the
                // group. Otherwise, drop the message with minimum
                // timestamp and retry.
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
                // Case: All buffers have at least 2 messages and not
                // all buffers are full.

                // Consume a message from the input stream.
                let item = input_stream_mut.as_mut().poll_next(ctx);

                let (key, item) = match item {
                    Ready(Some(Ok(item))) => item,
                    Ready(Some(Err(err))) => {
                        input_stream.set(None);
                        break Some(Err(err));
                    }
                    Ready(None) => {
                        input_stream.set(None);
                        break None;
                    }
                    Pending => {
                        return Pending;
                    }
                };

                // Try to insert the message to one of the buffer.  If
                // not successful, emit a feedback to tell the input
                // stream to catch up.
                if !state.push(key, item) {
                    // debug!("drop a late message for device {:?}", device);
                    state.update_feedback();
                    continue;
                }

                // Try to group up messages.
                let matching = state.try_match();

                // Emit a feedback.
                state.update_feedback();

                // Emit the group if a group is successfully formed.
                if let Some(matching) = matching {
                    break Some(Ok(matching));
                }
            }
        }
    } else {
        // Case: the input stream is depleted.

        // Loop until a valid group is found.
        loop {
            if state.is_empty() {
                break None;
            } else if let Some(matching) = state.try_match() {
                break Some(Ok(matching));
            } else {
                state.drop_min();
            }
        }
    };

    Ready(group)
}

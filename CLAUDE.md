# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Multi-Stream Synchronizer is a Rust library that implements time synchronization algorithms to group messages from multiple streams within a time window. Messages within each group come from distinct streams, identified by keys.

## Common Development Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Format code according to project standards
cargo fmt

# Check code without building
cargo check

# Build and view documentation
cargo doc --open

# Run the example
cargo run --example simple

# Clean build artifacts
cargo clean
```

## Architecture

The library follows a stream-based architecture with the following core components:

### Core Modules

- **sync.rs** (`src/sync.rs`): Main synchronization function that consumes input streams and produces grouped output
  - `sync()` function takes a stream of `(Key, Message)` pairs and returns an output stream and feedback receiver
  - Uses internal polling mechanism to manage message buffering and grouping
  - Handles stream depletion and error propagation

- **state.rs** (`src/state.rs`): Internal state management for the synchronizer
  - `State<K, T>` struct maintains buffers for each key, commit timestamps, and feedback channels
  - Tracks window size and buffer limits
  - Manages message matching and dropping logic

- **buffer.rs** (`src/buffer.rs`): Per-stream message buffering with timestamp ordering
  - `Buffer<T>` uses `VecDeque` for efficient front/back operations
  - Maintains monotonic timestamp ordering
  - Capacity-limited with configurable buffer sizes

- **types.rs** (`src/types.rs`): Core trait definitions and type aliases
  - `WithTimestamp` trait: Messages must implement to provide timestamp access
  - `Key` trait: Stream identifiers must be cloneable, hashable, and comparable
  - `Feedback<K>` struct: Controls input stream pacing via accepted timestamps and keys

- **config.rs** (`src/config.rs`): Configuration parameters
  - `window_size`: Time span for message grouping
  - `start_time`: Optional minimum timestamp filter
  - `buf_size`: Maximum messages per stream buffer (minimum 2)

### Key Algorithms

The synchronization works by:
1. Buffering messages from multiple streams in separate queues per key
2. Attempting to find message groups where all keys have messages within a time window
3. Using feedback mechanism to control input stream rates
4. Dropping oldest messages when buffers are full and no matches can be formed

### Message Flow

1. Input stream provides `(Key, Message)` pairs
2. Messages are buffered per key with timestamp validation
3. When buffers are ready (at least 2 messages per key), matching attempts occur
4. Successful matches produce `IndexMap<Key, Message>` output groups
5. Feedback controls upstream message flow via accepted timestamps

## Usage Pattern

Messages must implement `WithTimestamp` trait. Keys can be any type implementing the `Key` trait (automatically implemented for suitable types). The main entry point is `sync()` which returns both an output stream of grouped messages and a feedback receiver for flow control.

## Dependencies

- `futures`: Async stream processing
- `tokio`: Async runtime and synchronization primitives
- `indexmap`: Ordered hash maps for deterministic key ordering
- `eyre`: Error handling
- `tracing`: Logging and instrumentation
- `chrono`: Time handling utilities
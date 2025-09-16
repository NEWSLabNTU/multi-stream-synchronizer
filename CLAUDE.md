# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Multi-Stream Synchronizer is a Rust library that implements time synchronization algorithms to group messages from multiple streams within a time window. Messages within each group come from distinct streams, identified by keys. The library includes advanced staleness detection to prevent messages from accumulating indefinitely in real-time scenarios.

## Common Development Commands

```bash
# Build the project
cargo build

# Run tests (basic functionality)
cargo test

# Run tests with tokio features (includes staleness tests)
cargo test --features tokio

# Format and lint code (ALWAYS run before commits)
make lint

# Check code without building
cargo check

# Build and view documentation
cargo doc --open

# Clean build artifacts
cargo clean
```

## Architecture

The library follows a stream-based architecture with the following core components:

### Core Modules

- **lib.rs** (`src/lib.rs`): Public API and main synchronization function
  - `sync()` function takes a stream of `(Key, Message)` pairs and returns grouped output stream and feedback
  - Handles configuration validation and feature gate checks
  - Main entry point for users

- **state.rs** (`src/state.rs`): Core synchronization state machine
  - `State<K, T>` struct maintains buffers, manages time windows, and coordinates message matching
  - `try_match()` method implements the core synchronization algorithm
  - Handles message dropping when windows become too large
  - Integrates with staleness detection system

- **buffer.rs** (`src/buffer.rs`): Per-stream message buffering
  - `Buffer<T>` uses `VecDeque` for efficient FIFO operations
  - Supports message expiration with `drop_expired()` method
  - Capacity-limited with configurable buffer sizes
  - Maintains timestamp ordering within each stream

- **staleness.rs** (`src/staleness.rs`): Advanced staleness detection system
  - `StalenessDetector<K, T>` prevents indefinite message accumulation
  - Uses hybrid min-heap + timer wheel architecture for efficient expiration
  - Supports both immediate (tokio-based) and lazy checking modes
  - Configurable constraints: heap size, time horizon, precision gap

- **config.rs** (`src/config.rs`): Configuration types and presets
  - `Config`: Basic synchronization parameters (window_size, start_time, buf_size)
  - `StalenessConfig`: Advanced staleness detection parameters with presets
  - Preset configurations: `high_frequency()`, `low_frequency()`, `batch_processing()`

### Key Algorithms

**Core Synchronization:**
1. Buffer messages from multiple streams in separate queues per key
2. Calculate window boundaries using `inf_timestamp()` and `sup_timestamp()`
3. Attempt to form groups when all streams have messages within time window
4. Drop oldest messages when windows become too large
5. Use feedback mechanism to control input stream rates

**Staleness Detection:**
1. Track message expiration times in constrained min-heap (near-term) and timer wheel (overflow)
2. Schedule precise expiration checks using tokio timers (when feature enabled)
3. Remove expired messages proactively or during buffer access (lazy mode)
4. Prevent memory buildup when streams become imbalanced

### Message Flow

1. Input stream provides `(Key, Message)` pairs
2. Messages are added to per-key buffers with staleness tracking
3. Synchronization engine attempts to match messages within time windows
4. Successful matches produce `IndexMap<Key, Message>` output groups
5. Expired messages are cleaned up by staleness detector
6. Feedback controls upstream message flow

## Feature Gates

- **Default**: Basic synchronization without immediate staleness expiration
- **`tokio`**: Enables immediate staleness expiration with precise async timing
  - Required for `StalenessConfig::enable_immediate_expiration = true`
  - Provides sub-millisecond expiration precision
  - Used in real-time applications where bounded latency is critical

## Testing Strategy

The project has comprehensive test coverage across multiple categories:

### Test Files
- **`tests/basic_tests.rs`**: Core synchronization functionality
- **`tests/staleness_basic_tests.rs`**: Staleness detection without tokio
- **`tests/staleness_tokio_tests.rs`**: Advanced staleness tests with async timing

### Test Categories
- **Unit Tests**: Individual component behavior
- **Integration Tests**: End-to-end synchronization scenarios
- **Stress Tests**: High-volume, high-frequency message processing (1000+ msgs)
- **Performance Tests**: Throughput and latency validation

## Configuration Guidelines

### Basic Usage
```rust
// Simple time-window synchronization
let config = Config::basic(
    Duration::from_millis(100),  // 100ms windows
    None,                        // No start time filter
    64                           // 64 messages per buffer
);
```

### Real-Time Applications
```rust
// High-frequency real-time processing
let staleness_config = StalenessConfig::high_frequency(); // 100μs precision
let config = Config::with_staleness(window_size, None, buf_size, staleness_config);
```

### Batch Processing
```rust
// Memory-efficient batch processing
let staleness_config = StalenessConfig::batch_processing(); // Lazy checking
let config = Config::with_staleness(window_size, None, buf_size, staleness_config);
```

## Performance Considerations

- **Target Use Case**: Real-time applications requiring low latency and high throughput
- **Memory Management**: Staleness detection ensures bounded memory usage
- **CPU Efficiency**: Constrained heap and timer wheel minimize overhead
- **Scalability**: Handles thousands of messages per second per stream

## Dependencies

- **futures**: Async stream processing and utilities
- **tokio**: Async runtime, timers, and synchronization (optional feature)
- **indexmap**: Ordered hash maps for deterministic output
- **itertools**: Iterator utilities for stream processing
- **chrono**: Time handling and timestamp utilities
- **tracing**: Logging and instrumentation
- **eyre**: Error handling and reporting
- **flume**: Async channels for feedback mechanism
- **collected**: Additional collection utilities
- **humantime-serde**: Human-readable time serialization

## Documentation Structure

- **README.md**: User-focused documentation with examples and quick start
- **ALGORITHM.md**: Technical deep-dive into synchronization algorithm internals
- **CONTRIBUTING.md**: Developer guidelines, testing strategy, and contribution workflow
- **IMPROVE.md**: Enhancement roadmap and planned improvements (not included in crate)

## Common Development Patterns

### Adding New Tests
```rust
#[tokio::test]
async fn test_new_feature() {
    let config = StalenessConfig::high_frequency();
    // Test implementation
}
```

### Performance Benchmarking
```bash
# Always validate performance impact
cargo test --features tokio -- --nocapture | grep -E "(throughput|latency)"
```

### Error Handling
- Use `eyre::Result` for error propagation
- Provide meaningful error context
- Handle feature gate requirements gracefully

## Version Information

- **Current Version**: 0.2.0
- **License**: MIT OR Apache-2.0 dual licensing
- **MSRV**: Rust 1.70+ (stable toolchain)
- **Repository**: https://github.com/jerry73204/multi-stream-synchronizer
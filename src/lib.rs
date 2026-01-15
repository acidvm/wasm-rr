//! wasm-rr: Deterministic record-replay for WebAssembly components
//!
//! This library provides the core functionality for recording and replaying
//! non-deterministic host calls in WebAssembly components.

/// Trace event types for recording and replay
pub mod trace;

/// Utility functions
pub mod util;

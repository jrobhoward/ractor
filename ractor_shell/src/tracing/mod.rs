//! Actor tracing support for the ractor shell.
//!
//! This module provides functionality to trace actor message flow and lifecycle events,
//! similar to Erlang's `dbg` and trace BIFs.
//!
//! ## Usage
//!
//! From the shell:
//! ```text
//! ractor@local > trace worker_*     # Trace actors matching pattern
//! ractor@local > trace              # List active traces
//! ractor@local > trace off          # Stop all tracing
//! ractor@local > trace-to-file /tmp/trace.log worker_*
//! ```
//!
//! ## How It Works
//!
//! The tracing system hooks into Rust's `tracing` crate, which ractor uses internally
//! for span-based instrumentation. When an actor processes a message, ractor creates
//! spans with the actor's ID and name as fields.
//!
//! This module provides:
//! - [`ShellTracingLayer`]: A tracing layer that captures actor events
//! - [`TraceFilter`]: Pattern-based filtering for actor names
//! - [`TraceOutput`]: Formatted output to console or file

mod filter;
mod layer;
mod output;

pub use filter::{MinLevel, TraceFilter};
pub use layer::{ShellTracingLayer, TracingHandle};
pub use output::{TraceEvent, TraceEventType, TraceOutput, TraceOutputFormat};

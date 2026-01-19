//! Cluster demo module
//!
//! This example module contains all the code needed to run a Raft cluster demo:
//! - `raft` - Raft leader election implementation
//! - `raft_supervisor` - Supervision for Raft nodes with automatic restart
//! - `actors` - Demo actors (simple, dynamic, panicky)
//!
//! This is example code demonstrating ractor_shell capabilities, not part of the library.

pub mod actors;
pub mod raft;
pub mod raft_supervisor;

// Re-export commonly used items for convenience
pub use actors::{DemoActor, DemoMessage, DynamicDemoActor};

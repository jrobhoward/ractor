//! Cluster demo module
//!
//! This example module contains all the code needed to run a Raft cluster demo:
//! - `raft` - Raft leader election implementation
//! - `actors` - Demo actors (simple, dynamic, panicky)
//!
//! This is example code demonstrating ractor_shell capabilities, not part of the library.

pub mod actors;
pub mod raft;

// Re-export commonly used items for convenience
pub use actors::{DemoActor, DemoMessage, DynamicDemoActor};
pub use raft::{RaftConfig, RaftNode, RAFT_CLUSTER_GROUP};

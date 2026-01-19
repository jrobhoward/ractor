//! Shell Commands Module
//!
//! This module contains all shell command implementations, organized by category:
//!
//! - [`parse`]: Command parsing and the [`ShellCommand`] enum
//! - [`help`]: Help command implementation
//! - [`actors`]: Actor discovery commands (actors, registry, info, schema, pg)
//! - [`messaging`]: Message sending commands (send, call, send-file)
//! - [`cluster`]: Cluster and connection commands (connect, disconnect, nodes, use, cluster, ping)
//! - [`tracing`]: Tracing commands (trace, trace-to-file, trace-remote)
//! - [`monitoring`]: Actor monitoring commands (monitor, unmonitor, monitors)
//! - [`supervision`]: Supervision tree commands (tree, supervtree, parent)
//! - [`util`]: Utility commands (stats, load, stop, exit, top)

mod actors;
mod cluster;
mod help;
mod messaging;
mod monitoring;
mod parse;
mod supervision;
#[path = "tracing_cmds.rs"]
mod tracing_cmds;
mod util;

pub use parse::{resolve_alias, ShellCommand};

// Command implementations are provided via `impl ShellState` blocks in each module.
// The modules extend ShellState with their command handlers, so they don't export
// standalone items but they do need to be included in the compilation.

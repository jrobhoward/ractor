//! Error types for ractor_shell.
//!
//! This module provides strongly-typed errors for shell operations,
//! following the `thiserror` best practices documented in `SKILLS.md`.

use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during shell command execution.
#[derive(Debug, Error)]
pub enum ShellError {
    /// Actor was not found in the registry.
    #[error(
        "Actor '{0}' not found in registry. Use 'registry' command to list registered actors."
    )]
    ActorNotFound(String),

    /// Actor was not found on a remote node.
    #[error("Actor '{actor}' not found on node '{node}'.")]
    RemoteActorNotFound { actor: String, node: String },

    /// Not connected to the specified node.
    #[error("Not connected to node '{0}'. Use 'nodes' to see connected nodes.")]
    NodeNotConnected(String),

    /// Empty command entered.
    #[error("Empty command")]
    EmptyCommand,

    /// Unknown command entered.
    #[error("Unknown command: '{0}'. Type 'help' to see available commands.")]
    UnknownCommand(String),

    /// Missing required argument for a command.
    #[error("{command} requires {requirement}. Type 'help {command}' for usage.")]
    MissingArgument {
        command: &'static str,
        requirement: &'static str,
    },

    /// Unknown subcommand for a parent command.
    #[error("Unknown {parent} subcommand: '{subcommand}'. Type 'help {parent}' for usage.")]
    UnknownSubcommand {
        parent: &'static str,
        subcommand: String,
    },

    /// Invalid argument value for a command.
    #[error("{command}: {message}")]
    InvalidArgument {
        command: &'static str,
        message: String,
    },

    /// RPC call timed out.
    #[error("RPC call timed out after {0:?}")]
    RpcTimeout(Duration),

    /// RPC sender error (channel closed).
    #[error("RPC sender error (actor may have stopped)")]
    RpcSenderError,

    /// Failed to spawn an actor.
    #[error("Failed to spawn actor: {0}")]
    SpawnError(#[from] ractor::SpawnErr),

    /// Failed to send a message to an actor.
    #[error("Failed to send message: {0}")]
    MessagingError(String),

    /// No introspection actor found on remote node.
    #[error(
        "No introspection actor found on remote node. \
         Make sure the target node has spawned an IntrospectionActor."
    )]
    NoIntrospectionActor,

    /// Connection failed to remote node.
    #[error("Connection failed to {host}: {message}")]
    ConnectionFailed { host: String, message: String },

    /// Failed to read a file.
    #[error("Failed to read file '{path}': {message}")]
    FileReadError { path: String, message: String },

    /// JSON parsing error.
    #[error("JSON parse error: {0}. Example: {{\"command\": \"value\"}}")]
    JsonParseError(String),

    /// General parse error.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Feature not yet implemented.
    #[error("{0}")]
    NotImplemented(&'static str),

    /// Remote operation failed.
    #[error("Remote operation failed: {0}")]
    RemoteOperationFailed(String),

    /// Cluster connection error.
    #[error("Cluster error: {0}")]
    ClusterError(String),

    /// IO error during file operations.
    #[error("IO error during {operation} at '{path}': {source}")]
    IoError {
        operation: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Result type alias for shell operations.
pub type ShellResult<T> = Result<T, ShellError>;

impl ShellError {
    /// Create an RpcTimeout error with the default timeout duration.
    pub fn rpc_timeout() -> Self {
        ShellError::RpcTimeout(crate::DEFAULT_RPC_TIMEOUT)
    }

    /// Create a MessagingError from any MessagingErr type.
    pub fn messaging<T: std::fmt::Debug>(err: ractor::MessagingErr<T>) -> Self {
        ShellError::MessagingError(format!("{:?}", err))
    }
}

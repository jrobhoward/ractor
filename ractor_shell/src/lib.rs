//! # Ractor Shell
//!
//! An interactive REPL (Read-Eval-Print Loop) for debugging and observing Ractor actor systems,
//! inspired by Erlang's `erl` shell.
//!
//! ## Features
//!
//! - **Actor Introspection**: List and inspect actors via registry and process groups
//! - **Remote Connections**: Connect to distributed `ractor_cluster` nodes
//! - **Dynamic Messages**: Send arbitrary JSON messages to actors implementing [`dynamic::DynamicMessage`]
//! - **Monitoring**: Track actor lifecycle events (start, stop, panic, failure)
//! - **Cluster Topology**: Visualize mesh topology and cross-cluster process groups
//! - **Tab Completion**: Context-aware command and argument completion
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ractor_shell::{ShellState, ShellCommand};
//! use rustyline::Editor;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut state = ShellState::new().await?;
//!     let mut rl = Editor::<(), _>::new()?;
//!
//!     loop {
//!         let prompt = state.build_prompt();
//!         match rl.readline(&prompt) {
//!             Ok(line) => {
//!                 if let Ok(cmd) = ShellCommand::parse_line(&line) {
//!                     state.execute(cmd).await?;
//!                 }
//!             }
//!             Err(_) => break,
//!         }
//!         if state.should_exit {
//!             break;
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Modules
//!
//! - [`commands`]: Shell command parsing and execution
//! - [`config`]: Configuration file support
//! - [`dynamic`]: Dynamic message interface for shell-to-actor communication
//! - [`introspection`]: Remote introspection actor for cross-node queries
//! - [`monitor`]: Actor lifecycle monitoring
//! - [`protocol`]: Shell protocol messages for cluster communication
//! - [`completer`]: Tab completion support

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ractor::{Actor, ActorRef};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub mod commands;
pub mod completer;
pub mod completer_custom;
pub mod config;
pub mod dynamic;
pub mod error;
pub mod introspection;
pub mod messages;
pub mod monitor;
pub mod protocol;
pub mod schema_registry;
pub mod table;
pub mod tracing;
pub mod tui;

pub use commands::ShellCommand;
pub use error::{ShellError, ShellResult};

// ==================== Constants ====================

/// Default timeout for RPC calls.
pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Known process groups for local enumeration (ractor 0.15 doesn't expose group listing).
/// Note: "raft_cluster" is only relevant for examples that use Raft.
pub const KNOWN_PROCESS_GROUPS: &[&str] =
    &["ping_pong", "ractor_shell_introspection", "demo_group"];

use protocol::{ClusterTopology, ShellProtocolMessage, SubscriptionId};
use tracing::MinLevel;

/// Check if an event at the given level should be displayed based on the minimum level.
/// Returns true if the event level is >= min_level in severity.
#[allow(dead_code)] // Used by tests and commands module
pub(crate) fn should_display_level(event_level: &MinLevel, min_level: &MinLevel) -> bool {
    // MinLevel order (least to most severe): Trace, Debug, Info, Warn, Error
    // Event should display if its severity >= min_level severity
    let event_severity = match event_level {
        MinLevel::Trace => 0,
        MinLevel::Debug => 1,
        MinLevel::Info => 2,
        MinLevel::Warn => 3,
        MinLevel::Error => 4,
    };
    let min_severity = match min_level {
        MinLevel::Trace => 0,
        MinLevel::Debug => 1,
        MinLevel::Info => 2,
        MinLevel::Warn => 3,
        MinLevel::Error => 4,
    };
    event_severity >= min_severity
}

/// Active remote trace subscription
pub(crate) struct RemoteTraceSubscription {
    /// Node being traced
    pub(crate) node: String,
    /// Subscription ID from the remote node
    pub(crate) subscription_id: SubscriptionId,
    /// Pattern being traced
    pub(crate) pattern: String,
    /// Background polling task
    pub(crate) _poll_task: JoinHandle<()>,
}

/// Main shell state
pub struct ShellState {
    /// Flag to exit the REPL
    pub should_exit: bool,
    /// Current node context (None = local)
    pub current_node: Option<String>,
    /// Local node name for cluster participation
    pub local_node_name: String,
    /// NodeServer actor for cluster participation (spawned on first connect)
    pub node_server: Option<ActorRef<ractor_cluster::NodeServerMessage>>,
    /// Map of connected remote nodes to their introspection actors
    pub connected_nodes: HashMap<String, ActorRef<ShellProtocolMessage>>,
    /// Cached cluster topology (refreshed on demand)
    pub cluster_topology: Option<ClusterTopology>,
    /// Monitor actor for tracking actor lifecycle events
    pub monitor_actor: Option<ActorRef<monitor::MonitorMessage>>,
    /// Handle for controlling actor tracing
    pub tracing_handle: Option<tracing::TracingHandle>,
    /// Active remote trace subscriptions
    pub(crate) remote_trace_subscriptions: Arc<Mutex<Vec<RemoteTraceSubscription>>>,
    /// Quiet mode - suppresses verbose output (for scripting)
    pub quiet: bool,
}

impl ShellState {
    /// Create a new shell state with default configuration.
    ///
    /// This spawns the monitor actor and initializes the local node name
    /// based on the system hostname.
    pub async fn new() -> ShellResult<Self> {
        Self::with_options(false).await
    }

    /// Create a new shell state with quiet mode for scripting.
    ///
    /// When `quiet` is true, verbose startup messages are suppressed.
    pub async fn new_quiet() -> ShellResult<Self> {
        Self::with_options(true).await
    }

    /// Create a new shell state with options.
    async fn with_options(quiet: bool) -> ShellResult<Self> {
        let local_node_name = format!(
            "shell@{}",
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "localhost".to_string())
        );

        // Spawn monitor actor with quiet flag
        let (monitor_ref, _) = Actor::spawn(
            Some("shell_monitor".to_string()),
            monitor::MonitorActor,
            monitor::MonitorArgs { quiet },
        )
        .await?;

        Ok(Self {
            should_exit: false,
            current_node: None,
            local_node_name,
            node_server: None,
            connected_nodes: HashMap::new(),
            cluster_topology: None,
            monitor_actor: Some(monitor_ref),
            tracing_handle: None, // Initialized on first trace command
            remote_trace_subscriptions: Arc::new(Mutex::new(Vec::new())),
            quiet,
        })
    }

    /// Build the shell prompt string showing current node context.
    ///
    /// Returns a prompt like `ractor@local > ` or `ractor@node_name > `.
    ///
    /// Note: Colors are disabled in the prompt because ANSI escape sequences
    /// cause cursor positioning issues with rustyline on Windows terminals.
    /// The \x01/\x02 markers that normally fix this don't work reliably
    /// across all Windows terminal configurations.
    pub fn build_prompt(&self) -> String {
        let node_indicator = match &self.current_node {
            Some(node) => format!("@{}", node),
            None => "@local".to_string(),
        };

        format!("ractor{} > ", node_indicator)
    }

    /// Execute a shell command.
    ///
    /// This is the main dispatch method that routes commands to their handlers.
    /// Command implementations are in the [`commands`] module.
    pub async fn execute(&mut self, cmd: ShellCommand) -> ShellResult<()> {
        match cmd {
            ShellCommand::Help { command } => self.cmd_help(command).await,
            ShellCommand::Exit => self.cmd_exit().await,
            ShellCommand::Actors => self.cmd_actors().await,
            ShellCommand::Registry => self.cmd_registry().await,
            ShellCommand::PgList => self.cmd_pg_list().await,
            ShellCommand::PgMembers { group } => self.cmd_pg_members(group).await,
            ShellCommand::Info { actor } => self.cmd_info(actor).await,
            ShellCommand::Send { actor, message } => self.cmd_send(actor, message).await,
            ShellCommand::Call { actor, message } => self.cmd_call(actor, message).await,
            ShellCommand::Stop { actor } => self.cmd_stop(actor).await,
            ShellCommand::Connect { host } => self.cmd_connect(host).await,
            ShellCommand::Disconnect { node } => self.cmd_disconnect(node).await,
            ShellCommand::Reconnect { node } => self.cmd_reconnect(node).await,
            ShellCommand::Nodes => self.cmd_nodes().await,
            ShellCommand::Use { node } => self.cmd_use(node).await,
            ShellCommand::Cluster { subcommand } => self.cmd_cluster(subcommand).await,
            ShellCommand::Stats => self.cmd_stats().await,
            ShellCommand::Tree { actor } => self.cmd_tree(actor).await,
            ShellCommand::Supervtree { actor } => self.cmd_supervtree(actor).await,
            ShellCommand::Parent { actor } => self.cmd_parent(actor).await,
            ShellCommand::SendFile { actor, file_path } => {
                self.cmd_send_file(actor, file_path).await
            }
            ShellCommand::Load { script_path } => self.cmd_load(script_path).await,
            ShellCommand::Monitor { actor } => self.cmd_monitor(actor).await,
            ShellCommand::Unmonitor { actor } => self.cmd_unmonitor(actor).await,
            ShellCommand::Monitors => self.cmd_monitors().await,
            ShellCommand::MonitorEvents => self.cmd_monitor_events().await,
            ShellCommand::Top => self.cmd_top().await,
            ShellCommand::Trace { pattern } => self.cmd_trace(pattern).await,
            ShellCommand::TraceOff => self.cmd_trace_off().await,
            ShellCommand::TraceLevel { level } => self.cmd_trace_level(level).await,
            ShellCommand::TraceToFile { path, pattern } => {
                self.cmd_trace_to_file(path, pattern).await
            }
            ShellCommand::TraceRemote { node, pattern } => {
                self.cmd_trace_remote(node, pattern).await
            }
            ShellCommand::TraceRemoteOff => self.cmd_trace_remote_off().await,
            ShellCommand::Schema { actor, show_all } => self.cmd_schema(actor, show_all).await,
            ShellCommand::Ping { node } => self.cmd_ping(node).await,
        }
    }
}

#[cfg(test)]
mod lib_tests;

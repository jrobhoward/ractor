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
//! - [`config`]: Configuration file support
//! - [`dynamic`]: Dynamic message interface for shell-to-actor communication
//! - [`introspection`]: Remote introspection actor for cross-node queries
//! - [`monitor`]: Actor lifecycle monitoring
//! - [`protocol`]: Shell protocol messages for cluster communication
//! - [`completer`]: Tab completion support

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use colored::Colorize;
use ractor::rpc::CallResult;
use ractor::{pg, registry, Actor, ActorRef};
use tabled::Tabled;
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

pub use error::{ShellError, ShellResult};

// ==================== Constants ====================

/// Default timeout for RPC calls.
pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Known process groups for local enumeration (ractor 0.15 doesn't expose group listing).
/// Note: "raft_cluster" is only relevant for examples that use Raft.
pub const KNOWN_PROCESS_GROUPS: &[&str] =
    &["ping_pong", "ractor_shell_introspection", "demo_group"];

use introspection::INTROSPECTION_GROUP;
use protocol::{ClusterTopology, ShellProtocolMessage, SubscriptionId};
use tracing::MinLevel;

/// Check if an event at the given level should be displayed based on the minimum level.
/// Returns true if the event level is >= min_level in severity.
fn should_display_level(event_level: &MinLevel, min_level: &MinLevel) -> bool {
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
    node: String,
    /// Subscription ID from the remote node
    subscription_id: SubscriptionId,
    /// Pattern being traced
    pattern: String,
    /// Background polling task
    _poll_task: JoinHandle<()>,
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
    /// Returns a colored prompt like `ractor@local > ` or `ractor@node_name > `.
    pub fn build_prompt(&self) -> String {
        let node_indicator = match &self.current_node {
            Some(node) => format!("@{}", node),
            None => "@local".to_string(),
        };

        format!(
            "{}{} > ",
            "ractor".bright_cyan().bold(),
            node_indicator.bright_yellow()
        )
    }

    /// Execute a shell command.
    ///
    /// This is the main dispatch method that routes commands to their handlers.
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
            ShellCommand::Schema { actor } => self.cmd_schema(actor).await,
            ShellCommand::Ping { node } => self.cmd_ping(node).await,
        }
    }

    /// Launch the interactive TUI dashboard.
    async fn cmd_top(&mut self) -> ShellResult<()> {
        // Run the TUI - local or remote depending on current connection
        if let Some(ref node) = self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node) {
                println!(
                    "{}",
                    format!(
                        "Launching actor dashboard for {}... (press 'q' to exit)",
                        node
                    )
                    .cyan()
                );
                if let Err(e) = tui::App::run_remote(node.clone(), introspection_ref.clone()).await
                {
                    eprintln!("{} {}", "TUI error:".red(), e);
                }
            } else {
                return Err(ShellError::NodeNotConnected(node.clone()));
            }
        } else {
            println!(
                "{}",
                "Launching actor dashboard... (press 'q' to exit)".cyan()
            );
            if let Err(e) = tui::App::run().await {
                eprintln!("{} {}", "TUI error:".red(), e);
            }
        }

        Ok(())
    }

    /// Start tracing actors or list active traces.
    async fn cmd_trace(&mut self, pattern: Option<String>) -> ShellResult<()> {
        // Initialize tracing handle if not already done
        if self.tracing_handle.is_none() {
            let (layer, handle) = tracing::ShellTracingLayer::new();

            // Install the layer with tracing-subscriber
            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::util::SubscriberInitExt;

            // Try to set global subscriber - may fail if already set
            let _ = tracing_subscriber::registry().with(layer).try_init();

            self.tracing_handle = Some(handle);

            if !self.quiet {
                println!("{} Tracing system initialized", "✓".green());
            }
        }

        let handle = self.tracing_handle.as_ref().unwrap();

        match pattern {
            Some(pat) => {
                handle.trace(&pat);
                println!("{} Tracing actors matching: {}", "✓".green(), pat.cyan());

                // Show all active patterns
                let patterns = handle.patterns();
                if patterns.len() > 1 {
                    println!("{}", "Active trace patterns:".bright_black());
                    for p in patterns {
                        println!("  {}", p.cyan());
                    }
                }
            }
            None => {
                // List active traces (local and remote)
                let remote_subs = self.remote_trace_subscriptions.lock().await;
                let has_local = handle.is_active();
                let has_remote = !remote_subs.is_empty();

                if !has_local && !has_remote {
                    println!(
                        "{}",
                        "No active traces. Use 'trace <pattern>' to start tracing.".bright_black()
                    );
                    println!("{}", "Examples:".bright_black());
                    println!(
                        "  {}     - trace actors starting with 'worker_'",
                        "trace worker_*".cyan()
                    );
                    println!("  {}            - trace all actors", "trace *".cyan());
                    println!(
                        "  {} - filter to INFO level and above",
                        "trace level INFO".cyan()
                    );
                    println!("  {}  - stop all tracing", "trace off".cyan());
                } else {
                    if has_local {
                        println!("{}", "Local trace patterns:".bold());
                        for p in handle.patterns() {
                            println!("  {}", p.cyan());
                        }
                        println!();
                    }

                    if has_remote {
                        println!("{}", "Remote trace subscriptions:".bold());
                        for sub in remote_subs.iter() {
                            println!(
                                "  {} {} (subscription {})",
                                sub.node.cyan(),
                                sub.pattern.bright_black(),
                                sub.subscription_id
                            );
                        }
                        println!();
                    }

                    println!("{}", "Minimum level:".bold());
                    println!("  {}", handle.min_level().to_string().cyan());
                    println!();
                    println!("{}", "Outputs:".bold());
                    for o in handle.outputs() {
                        println!("  {}", o.bright_black());
                    }
                }
            }
        }

        Ok(())
    }

    /// Stop all tracing.
    async fn cmd_trace_off(&mut self) -> ShellResult<()> {
        if let Some(handle) = &self.tracing_handle {
            let was_active = handle.is_active();
            handle.trace_off();
            handle.clear_file_outputs();

            if was_active {
                println!("{} Tracing stopped", "✓".green());
            } else {
                println!("{}", "No active traces".bright_black());
            }
        } else {
            println!("{}", "Tracing not initialized".bright_black());
        }

        Ok(())
    }

    /// Get or set the minimum trace log level.
    async fn cmd_trace_level(&mut self, level: Option<String>) -> ShellResult<()> {
        // Initialize tracing handle if not already done
        if self.tracing_handle.is_none() {
            let (layer, handle) = tracing::ShellTracingLayer::new();

            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::util::SubscriberInitExt;

            let _ = tracing_subscriber::registry().with(layer).try_init();
            self.tracing_handle = Some(handle);

            if !self.quiet {
                println!("{} Tracing system initialized", "✓".green());
            }
        }

        let handle = self.tracing_handle.as_ref().unwrap();

        match level {
            Some(level_str) => {
                let min_level: tracing::MinLevel =
                    level_str
                        .parse()
                        .map_err(|e: String| ShellError::InvalidArgument {
                            command: "trace level",
                            message: e,
                        })?;
                handle.set_min_level(min_level);
                println!(
                    "{} Minimum trace level set to: {}",
                    "✓".green(),
                    min_level.to_string().cyan()
                );
                println!(
                    "  {}",
                    "Events below this level will be filtered out.".bright_black()
                );
            }
            None => {
                let current_level = handle.min_level();
                println!("{}", "Current minimum trace level:".bold());
                println!("  {}", current_level.to_string().cyan());
                println!();
                println!(
                    "{}",
                    "Available levels (from most to least verbose):".bold()
                );
                println!("  {} - Show all events", "TRACE".bright_black());
                println!(
                    "  {} - Filter out TRACE, show DEBUG and above",
                    "DEBUG".blue()
                );
                println!(
                    "  {}  - Filter out TRACE/DEBUG, show INFO and above",
                    "INFO".green()
                );
                println!(
                    "  {}  - Filter out TRACE/DEBUG/INFO, show WARN and above",
                    "WARN".yellow()
                );
                println!("  {} - Show only ERROR events", "ERROR".red());
                println!();
                println!(
                    "{}",
                    "Usage: trace level <LEVEL>  (e.g., trace level INFO)".bright_black()
                );
            }
        }

        Ok(())
    }

    /// Log traces to a file.
    async fn cmd_trace_to_file(
        &mut self,
        path: String,
        pattern: Option<String>,
    ) -> ShellResult<()> {
        // Initialize tracing handle if not already done
        if self.tracing_handle.is_none() {
            let (layer, handle) = tracing::ShellTracingLayer::new();

            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::util::SubscriberInitExt;

            let _ = tracing_subscriber::registry().with(layer).try_init();
            self.tracing_handle = Some(handle);

            if !self.quiet {
                println!("{} Tracing system initialized", "✓".green());
            }
        }

        let handle = self.tracing_handle.as_ref().unwrap();

        // Add file output
        let path_buf = std::path::PathBuf::from(&path);
        handle
            .add_file_output(path_buf, tracing::TraceOutputFormat::Pretty)
            .map_err(|e| ShellError::IoError {
                operation: "create trace file",
                path: path.clone(),
                source: e,
            })?;

        // If pattern provided, start tracing
        if let Some(pat) = pattern {
            handle.trace(&pat);
            println!(
                "{} Tracing actors matching '{}' to file: {}",
                "✓".green(),
                pat.cyan(),
                path.bright_black()
            );
        } else if !handle.is_active() {
            // No pattern and no active traces - enable all
            handle.trace("*");
            println!(
                "{} Tracing all actors to file: {}",
                "✓".green(),
                path.bright_black()
            );
        } else {
            println!(
                "{} Added trace output file: {}",
                "✓".green(),
                path.bright_black()
            );
        }

        Ok(())
    }

    /// Start remote tracing on a connected node.
    async fn cmd_trace_remote(&mut self, node: String, pattern: String) -> ShellResult<()> {
        // Check if we're connected to this node
        let introspection_ref = self
            .connected_nodes
            .get(&node)
            .ok_or_else(|| ShellError::NodeNotConnected(node.clone()))?
            .clone();

        // Initialize tracing handle if not already done (needed for level filtering)
        if self.tracing_handle.is_none() {
            let (layer, handle) = tracing::ShellTracingLayer::new();

            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::util::SubscriberInitExt;

            let _ = tracing_subscriber::registry().with(layer).try_init();
            self.tracing_handle = Some(handle);
        }

        // Subscribe to traces on the remote node
        let result = introspection_ref
            .call(
                |reply| ShellProtocolMessage::SubscribeToTraces(pattern.clone(), reply),
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        let subscription_id = match result {
            CallResult::Success(id) => id,
            CallResult::SenderError => {
                return Err(ShellError::RemoteOperationFailed(format!(
                    "Remote introspection actor not available on {}",
                    node
                )));
            }
            CallResult::Timeout => {
                return Err(ShellError::RpcTimeout(DEFAULT_RPC_TIMEOUT));
            }
        };

        println!(
            "{} Subscribed to remote traces on {} (pattern: {}, subscription: {})",
            "✓".green(),
            node.yellow(),
            pattern.cyan(),
            subscription_id
        );

        // Spawn polling task
        let poll_node = node.clone();
        let poll_ref = introspection_ref.clone();
        let subscriptions = self.remote_trace_subscriptions.clone();
        let tracing_handle = self.tracing_handle.clone();

        let poll_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;

                // Poll for events
                match poll_ref
                    .call(
                        |reply| ShellProtocolMessage::PollTraces(subscription_id, reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                {
                    Ok(CallResult::Success(batch)) => {
                        // Get current min level for filtering
                        let min_level = tracing_handle
                            .as_ref()
                            .map(|h| h.min_level())
                            .unwrap_or_default();

                        // Display events
                        for event in &batch.events {
                            // Filter by level - parse the level string and check against min
                            let event_level: Result<tracing::MinLevel, _> = event.level.parse();
                            if let Ok(level) = event_level {
                                // Skip if event level is below minimum
                                // MinLevel order: Trace < Debug < Info < Warn < Error
                                // If min is Info, skip Trace and Debug
                                if !should_display_level(&level, &min_level) {
                                    continue;
                                }
                            }

                            let level_str = &event.level;
                            let level_colored = match level_str.as_str() {
                                "ERROR" => level_str.red(),
                                "WARN" => level_str.yellow(),
                                "INFO" => level_str.green(),
                                "DEBUG" => level_str.blue(),
                                "TRACE" => level_str.purple(),
                                _ => level_str.white(),
                            };

                            let actor_info = if let Some(name) = &event.actor_name {
                                format!("{}({})", name, event.actor_id.as_deref().unwrap_or("?"))
                            } else {
                                event.actor_id.as_deref().unwrap_or("?").to_string()
                            };

                            println!(
                                "[{}] {} {} {} {}",
                                poll_node.yellow(),
                                event.timestamp.bright_black(),
                                level_colored,
                                actor_info.cyan(),
                                event.message
                            );

                            // Show fields if present
                            if !event.fields.is_empty() {
                                for (key, value) in &event.fields {
                                    println!("      {}: {}", key.bright_black(), value);
                                }
                            }
                        }

                        // Warn if events were dropped
                        if batch.dropped_count > 0 {
                            println!(
                                "{} [{}] {} trace events dropped due to buffer overflow",
                                "⚠".yellow(),
                                poll_node.yellow(),
                                batch.dropped_count
                            );
                        }
                    }
                    Ok(CallResult::Timeout) | Ok(CallResult::SenderError) => {
                        // Connection lost, stop polling
                        eprintln!(
                            "{} Lost connection to {}, stopping remote trace",
                            "✗".red(),
                            poll_node.yellow()
                        );

                        // Remove subscription from list
                        let mut subs = subscriptions.lock().await;
                        subs.retain(|s| s.subscription_id != subscription_id);
                        break;
                    }
                    Err(_) => {
                        // Error polling, continue (transient errors are OK)
                    }
                }
            }
        });

        // Store subscription
        let subscription = RemoteTraceSubscription {
            node: node.clone(),
            subscription_id,
            pattern: pattern.clone(),
            _poll_task: poll_task,
        };

        self.remote_trace_subscriptions
            .lock()
            .await
            .push(subscription);

        Ok(())
    }

    /// Stop all remote tracing.
    async fn cmd_trace_remote_off(&mut self) -> ShellResult<()> {
        let mut subscriptions = self.remote_trace_subscriptions.lock().await;

        if subscriptions.is_empty() {
            println!("{}", "No active remote trace subscriptions".yellow());
            return Ok(());
        }

        let count = subscriptions.len();

        // Unsubscribe from each remote node
        for sub in subscriptions.drain(..) {
            if let Some(introspection_ref) = self.connected_nodes.get(&sub.node) {
                let _ = introspection_ref
                    .call(
                        |reply| {
                            ShellProtocolMessage::UnsubscribeFromTraces(sub.subscription_id, reply)
                        },
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await;
            }
            // Poll task will be dropped and cancelled when subscription is dropped
        }

        println!(
            "{} Stopped {} remote trace subscription{}",
            "✓".green(),
            count,
            if count == 1 { "" } else { "s" }
        );

        Ok(())
    }

    async fn cmd_help(&self, command: Option<String>) -> ShellResult<()> {
        if let Some(cmd) = command {
            // Show detailed help for specific command
            match cmd.as_str() {
                "actors" => {
                    println!("{}", "actors".green().bold());
                    println!("  List all actors in the current context");
                    println!("\n{}", "Usage:".bold());
                    println!("  actors");
                }
                "registry" => {
                    println!("{}", "registry".green().bold());
                    println!("  Show all named/registered actors");
                    println!("\n{}", "Usage:".bold());
                    println!("  registry");
                }
                "pg" => {
                    println!("{}", "pg".green().bold());
                    println!("  Process group commands");
                    println!("\n{}", "Usage:".bold());
                    println!("  pg list              List all process groups");
                    println!("  pg members <group>   Show members of a specific group");
                }
                "info" => {
                    println!("{}", "info <actor>".green().bold());
                    println!("  Show detailed information about an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  info <actor_name>");
                }
                "send" => {
                    println!("{}", "send <actor> <message>".green().bold());
                    println!("  Send a message to an actor (fire-and-forget)");
                    println!("\n{}", "Usage:".bold());
                    println!("  send <actor_name> <json_message>");
                }
                "call" => {
                    println!("{}", "call <actor> <message>".green().bold());
                    println!("  Send a message and wait for reply");
                    println!("\n{}", "Usage:".bold());
                    println!("  call <actor_name> <json_message>");
                }
                "stop" => {
                    println!("{}", "stop <actor>".green().bold());
                    println!("  Stop an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  stop <actor_name>");
                }
                "send-file" | "sendfile" => {
                    println!("{}", "send-file <actor> <file>".green().bold());
                    println!("  Send a message from a JSON file to an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  send-file <actor_name> <file_path>");
                    println!("\n{}", "Example:".bold());
                    println!("  send-file ping_pong messages/ping.json");
                }
                "load" => {
                    println!("{}", "load <script>".green().bold());
                    println!("  Execute shell commands from a script file");
                    println!("\n{}", "Usage:".bold());
                    println!("  load <script_path>");
                    println!("\n{}", "Example:".bold());
                    println!("  load scripts/setup.txt");
                    println!("\n{}", "Script Format:".bold());
                    println!("  - One command per line");
                    println!("  - Lines starting with # are comments");
                    println!("  - Empty lines are ignored");
                }
                "monitor" => {
                    println!("{}", "monitor <actor>".green().bold());
                    println!("  Start monitoring an actor's lifecycle events");
                    println!("\n{}", "Usage:".bold());
                    println!("  monitor <actor_name>   Monitor a specific actor");
                    println!(
                        "  monitor events         Poll and display monitor events (remote only)"
                    );
                    println!("\n{}", "Example:".bold());
                    println!("  monitor demo_actor_1");
                    println!("  monitor events");
                    println!("\n{}", "Note:".bold());
                    println!("  - Shows start, stop, panic, and kill events");
                    println!("  - Local: events are displayed automatically in real-time");
                    println!("  - Remote: use 'monitor events' to poll for events");
                }
                "unmonitor" => {
                    println!("{}", "unmonitor <actor>".green().bold());
                    println!("  Stop monitoring an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  unmonitor <actor_name>");
                    println!("\n{}", "Example:".bold());
                    println!("  unmonitor demo_actor_1");
                }
                "monitors" => {
                    println!("{}", "monitors".green().bold());
                    println!("  List all currently monitored actors");
                    println!("\n{}", "Usage:".bold());
                    println!("  monitors");
                }
                "top" => {
                    println!("{}", "top".green().bold());
                    println!("  Launch interactive TUI dashboard for actor monitoring");
                    println!("\n{}", "Usage:".bold());
                    println!("  top");
                    println!("\n{}", "Alias:".bold());
                    println!("  t");
                    println!("\n{}", "Keyboard Shortcuts:".bold());
                    println!("  q, Esc      Quit dashboard");
                    println!("  ↑/k, ↓/j    Navigate up/down");
                    println!("  s           Cycle sort column");
                    println!("  S           Toggle sort direction");
                    println!("  /           Enter filter mode");
                    println!("  c           Clear filter");
                    println!("  r           Force refresh");
                    println!("  ?           Show help");
                }
                "trace" => {
                    println!("{}", "trace [pattern]".green().bold());
                    println!("  Trace actor message flow and lifecycle events");
                    println!("\n{}", "Usage:".bold());
                    println!("  trace              List active traces and current settings");
                    println!("  trace <pattern>    Start tracing actors matching pattern");
                    println!("  trace off          Stop all tracing");
                    println!("  trace level        Show current minimum log level");
                    println!(
                        "  trace level <LVL>  Set minimum level (TRACE|DEBUG|INFO|WARN|ERROR)"
                    );
                    println!("\n{}", "Alias:".bold());
                    println!("  tr");
                    println!("\n{}", "Patterns:".bold());
                    println!("  worker_*     Match actors starting with 'worker_'");
                    println!("  *            Match all actors");
                    println!("  supervisor   Match exact name 'supervisor'");
                    println!("\n{}", "Log Levels (most to least verbose):".bold());
                    println!("  TRACE        Show all events (default)");
                    println!("  DEBUG        Filter out TRACE events");
                    println!("  INFO         Filter out TRACE and DEBUG events");
                    println!("  WARN         Show only WARN and ERROR events");
                    println!("  ERROR        Show only ERROR events");
                    println!("\n{}", "Examples:".bold());
                    println!("  trace worker_*     Start tracing worker actors");
                    println!("  trace *            Trace all actors");
                    println!("  trace level INFO   Show only INFO, WARN, ERROR events");
                    println!("  trace off          Stop tracing");
                    println!("\n{}", "Note:".bold());
                    println!("  Traces show actor span enter/exit and tracing events.");
                    println!("  Requires actors to use ractor's tracing instrumentation.");
                }
                "trace-to-file" | "tracefile" => {
                    println!("{}", "trace-to-file <path> [pattern]".green().bold());
                    println!("  Log actor traces to a file");
                    println!("\n{}", "Usage:".bold());
                    println!("  trace-to-file <path>           Log all traces to file");
                    println!("  trace-to-file <path> <pattern> Log matching traces to file");
                    println!("\n{}", "Alias:".bold());
                    println!("  tf");
                    println!("\n{}", "Examples:".bold());
                    println!("  trace-to-file /tmp/trace.log worker_*");
                    println!("  trace-to-file ./debug.log");
                    println!("\n{}", "Note:".bold());
                    println!("  File output is in addition to console output.");
                    println!("  Use 'trace off' to stop and close file outputs.");
                }
                "trace-remote" | "traceremote" => {
                    println!("{}", "trace remote <node> <pattern>".green().bold());
                    println!("  Subscribe to trace events from a remote node");
                    println!("\n{}", "Usage:".bold());
                    println!("  trace remote <node> <pattern>  Subscribe to remote traces");
                    println!("  trace remote off               Stop all remote subscriptions");
                    println!("\n{}", "Examples:".bold());
                    println!("  trace remote 127.0.0.1:9001 raft_*");
                    println!("  trace remote 127.0.0.1:9001 *");
                    println!("  trace remote off");
                    println!("\n{}", "Note:".bold());
                    println!("  Remote traces are displayed with [node] prefix.");
                    println!("  Events are polled periodically from the remote node.");
                    println!("  If buffer overflows, dropped count is reported.");
                }
                "ping" => {
                    println!("{}", "ping <node>".green().bold());
                    println!("  Ping a remote node to measure round-trip latency");
                    println!("\n{}", "Usage:".bold());
                    println!("  ping <host:port>");
                    println!("\n{}", "Examples:".bold());
                    println!("  ping 127.0.0.1:9001");
                    println!("\n{}", "Note:".bold());
                    println!("  Auto-connects if not already connected to the node.");
                    println!("  Returns latency in milliseconds.");
                }
                _ => {
                    println!("{} Unknown command: {}", "Error:".red().bold(), cmd);
                }
            }
        } else {
            // Show general help
            println!("{}", "Available Commands:".bold().underline());
            println!();
            println!("{}", "  Local Introspection:".bright_black());
            println!("  {}  Show this help message", "help [command]".green());
            println!("  {}            List all actors", "actors".green());
            println!("  {}          Show registered actors", "registry".green());
            println!("  {}            List process groups", "pg list".green());
            println!(
                "  {}    Show process group members",
                "pg members <group>".green()
            );
            println!("  {}       Show actor details", "info <actor>".green());
            println!("  {} Send a cast message", "send <actor> <msg>".green());
            println!("  {} Send an RPC message", "call <actor> <msg>".green());
            println!("  {}       Stop an actor", "stop <actor>".green());
            println!();
            println!("{}", "  Message & Script Input:".bright_black());
            println!(
                "  {} Send message from file",
                "send-file <actor> <file>".green()
            );
            println!("  {} Execute shell script", "load <script>".green());
            println!();
            println!("{}", "  Remote Connection:".bright_black());
            println!(
                "  {}  Connect to remote node",
                "connect <host:port>".green()
            );
            println!("  {} Disconnect from node", "disconnect <node>".green());
            println!("  {}             List connected nodes", "nodes".green());
            println!("  {}       Switch to a node context", "use <node>".green());
            println!(
                "  {}       Ping node and measure latency",
                "ping <node>".green()
            );
            println!();
            println!("{}", "  Cluster Topology:".bright_black());
            println!("  {}          Show cluster topology", "cluster".green());
            println!("  {}       Show cluster nodes", "cluster nodes".green());
            println!("  {}      Show process groups", "cluster groups".green());
            println!("  {}      Show all actors", "cluster actors".green());
            println!();
            println!("{}", "  System Introspection:".bright_black());
            println!("  {}             Show system statistics", "stats".green());
            println!("  {}       Show process group tree", "tree".green());
            println!(
                "  {}               Launch TUI actor dashboard",
                "top".green()
            );
            println!();
            println!("{}", "  Actor Monitoring:".bright_black());
            println!(
                "  {}    Start monitoring actor events",
                "monitor <actor>".green()
            );
            println!(
                "  {}  Stop monitoring an actor",
                "unmonitor <actor>".green()
            );
            println!("  {}          List monitored actors", "monitors".green());
            println!();
            println!("{}", "  Tracing:".bright_black());
            println!(
                "  {}   Trace actors (pattern or 'off')",
                "trace [pattern]".green()
            );
            println!("  {}  Log traces to file", "trace-to-file <path>".green());
            println!(
                "  {} Subscribe to remote traces",
                "trace remote <node> <pattern>".green()
            );
            println!();
            println!("  {}              Exit the shell", "exit".green());
            println!();
            println!("{}", "  Shell Features:".bright_black());
            println!(
                "  {} Use TAB to auto-complete commands, actors, and groups",
                "•".bright_cyan()
            );
            println!(
                "  {} Command history with UP/DOWN arrows",
                "•".bright_cyan()
            );
            println!(
                "  {} Aliases: {}, {}, {}, {}, {}, {}, {}",
                "•".bright_cyan(),
                "a=actors".bright_black(),
                "r=registry".bright_black(),
                "i=info".bright_black(),
                "s=send".bright_black(),
                "c=call".bright_black(),
                "t=top".bright_black(),
                "q=quit".bright_black()
            );
            println!();
            println!("Type 'help <command>' for more information on a specific command");
        }
        Ok(())
    }

    async fn cmd_exit(&mut self) -> ShellResult<()> {
        self.should_exit = true;
        Ok(())
    }

    async fn cmd_actors(&self) -> ShellResult<()> {
        #[derive(Tabled)]
        struct ActorRow {
            #[tabled(rename = "Name")]
            name: String,
            #[tabled(rename = "ID")]
            id: String,
            #[tabled(rename = "Status")]
            status: String,
        }

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListRegisteredActors,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        if actors.is_empty() {
                            println!("No registered actors on {}.", node_name);
                            return Ok(());
                        }

                        let mut rows = Vec::new();
                        for actor_info in actors {
                            rows.push(ActorRow {
                                name: actor_info.name.unwrap_or_else(|| "-".to_string()),
                                id: actor_info.id,
                                status: actor_info.status,
                            });
                        }

                        println!("{}", table::build_table(rows));
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            println!("{}", "Listing actors...".bright_black());
            println!("Note: Phase 1 only shows named actors. Full enumeration coming in Phase 2.");

            let registered = registry::registered();

            if registered.is_empty() {
                println!("No registered actors found.");
                return Ok(());
            }

            let mut rows = Vec::new();
            for name in registered {
                if let Some(cell) = registry::where_is(name.clone()) {
                    rows.push(ActorRow {
                        name: name.clone(),
                        id: cell.get_id().to_string(),
                        status: format!("{:?}", cell.get_status()),
                    });
                }
            }

            println!("{}", table::build_table(rows));
        }

        Ok(())
    }

    async fn cmd_registry(&self) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListRegisteredActors,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        if actors.is_empty() {
                            println!("No registered actors on {}.", node_name);
                            return Ok(());
                        }

                        println!(
                            "{}",
                            format!("Registered Actors on {} ({}):", node_name, actors.len())
                                .bold()
                        );
                        for actor_info in actors {
                            println!(
                                "  {} {} {}",
                                actor_info.name.unwrap_or_else(|| "-".to_string()).green(),
                                "→".bright_black(),
                                actor_info.id.bright_black()
                            );
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local registry query
            let registered = registry::registered();

            if registered.is_empty() {
                println!("No registered actors.");
                return Ok(());
            }

            println!(
                "{}",
                format!("Registered Actors ({}):", registered.len()).bold()
            );
            for name in registered {
                if let Some(cell) = registry::where_is(name.clone()) {
                    println!(
                        "  {} {} {}",
                        name.green(),
                        "→".bright_black(),
                        cell.get_id().to_string().bright_black()
                    );
                }
            }
        }

        Ok(())
    }

    async fn cmd_pg_list(&self) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListProcessGroups,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(groups) => {
                        if groups.is_empty() {
                            println!("No process groups found on {}", node_name);
                            return Ok(());
                        }

                        println!(
                            "{}",
                            format!("Process groups on {} ({} total):", node_name, groups.len())
                                .bold()
                        );
                        for group in groups {
                            println!("  - {}", group.green());
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
                return Ok(());
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution
        let groups = pg::which_groups();
        if groups.is_empty() {
            println!("No process groups found");
            println!();
            println!(
                "  {} Use 'pg members <group>' to query specific groups",
                "•".bright_black()
            );
            return Ok(());
        }

        println!(
            "{}",
            format!("Process groups ({} total):", groups.len()).bold()
        );
        for group in groups {
            println!("  - {}", group.green());
        }

        Ok(())
    }

    async fn cmd_pg_members(&self, group: String) -> ShellResult<()> {
        #[derive(Tabled)]
        struct MemberRow {
            #[tabled(rename = "Actor ID")]
            id: String,
            #[tabled(rename = "Name")]
            name: String,
            #[tabled(rename = "Local")]
            is_local: String,
        }

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetProcessGroupMembers(group.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        if actors.is_empty() {
                            println!(
                                "Process group '{}' has no members on {} (or doesn't exist)",
                                group, node_name
                            );
                            return Ok(());
                        }

                        println!(
                            "{}",
                            format!(
                                "Members of '{}' on {} ({}):",
                                group,
                                node_name,
                                actors.len()
                            )
                            .bold()
                        );

                        let mut rows = Vec::new();
                        for actor_info in actors {
                            rows.push(MemberRow {
                                id: actor_info.id,
                                name: actor_info.name.unwrap_or_else(|| "-".to_string()),
                                is_local: if actor_info.is_local { "yes" } else { "no" }
                                    .to_string(),
                            });
                        }

                        let table = table::build_table(rows);
                        println!("{}", table);
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            let members = pg::get_members(&group);

            if members.is_empty() {
                println!(
                    "Process group '{}' has no members (or doesn't exist)",
                    group
                );
                return Ok(());
            }

            println!(
                "{}",
                format!("Members of '{}' ({}):", group, members.len()).bold()
            );

            let mut rows = Vec::new();
            for cell in members {
                let id = cell.get_id();
                let name = cell.get_name().unwrap_or_else(|| "-".to_string());
                let is_local = if id.is_local() { "yes" } else { "no" };

                rows.push(MemberRow {
                    id: id.to_string(),
                    name,
                    is_local: is_local.to_string(),
                });
            }

            let table = table::build_table(rows);
            println!("{}", table);
        }

        Ok(())
    }

    async fn cmd_info(&self, actor: String) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetActorInfo(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(Some(actor_info)) => {
                        println!("{}", format!("Actor on {}: {}", node_name, actor).bold());
                        println!("  ID:     {}", actor_info.id);
                        println!("  Status: {}", actor_info.status);
                        if let Some(name) = actor_info.name {
                            println!("  Name:   {}", name);
                        }
                        println!(
                            "  Local:  {}",
                            if actor_info.is_local { "yes" } else { "no" }
                        );
                        Ok(())
                    }
                    CallResult::Success(None) => Err(ShellError::RemoteActorNotFound {
                        actor: actor.clone(),
                        node: node_name.clone(),
                    }),
                    CallResult::Timeout => Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => Err(ShellError::RpcSenderError),
                }
            } else {
                Err(ShellError::NodeNotConnected(node_name.clone()))
            }
        } else {
            // Local query
            if let Some(cell) = registry::where_is(actor.clone()) {
                println!("{}", format!("Actor: {}", actor).bold());
                println!("  ID:     {}", cell.get_id());
                println!("  Status: {:?}", cell.get_status());
                if let Some(name) = cell.get_name() {
                    println!("  Name:   {}", name);
                }

                // Show supervision information
                match cell.try_get_supervisor() {
                    Some(supervisor) => {
                        let supervisor_name = supervisor
                            .get_name()
                            .unwrap_or_else(|| format!("{}", supervisor.get_id()));
                        println!("  Supervisor: {}", supervisor_name.green());
                    }
                    None => {
                        println!("  Supervisor: {}", "None (root actor)".bright_black());
                    }
                }

                let children = cell.get_children();
                println!("  Children: {}", children.len());
                if !children.is_empty() {
                    for child in children.iter() {
                        let child_name = child
                            .get_name()
                            .unwrap_or_else(|| format!("{}", child.get_id()));
                        println!("    - {}", child_name.cyan());
                    }
                }

                Ok(())
            } else {
                Err(ShellError::ActorNotFound(actor))
            }
        }
    }

    /// Show message schema for an actor, or list all schema-enabled actors.
    async fn cmd_schema(&self, actor: Option<String>) -> ShellResult<()> {
        match actor {
            Some(actor_name) => {
                // Show schema for a specific actor
                if let Some(node_name) = &self.current_node {
                    // Remote query
                    if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                        let result = introspection_ref
                            .call(
                                |reply| {
                                    ShellProtocolMessage::GetMessageSchema(
                                        actor_name.clone(),
                                        reply,
                                    )
                                },
                                Some(DEFAULT_RPC_TIMEOUT),
                            )
                            .await
                            .map_err(ShellError::messaging)?;

                        match result {
                            CallResult::Success(Some(schema_json)) => {
                                self.display_schema(&actor_name, &schema_json);
                                Ok(())
                            }
                            CallResult::Success(None) => {
                                println!(
                                    "{} Actor '{}' does not have a registered schema.",
                                    "ℹ".cyan(),
                                    actor_name
                                );
                                println!(
                                    "{}",
                                    "  Hint: The actor may use DynamicMessage or not support schema introspection."
                                        .bright_black()
                                );
                                Ok(())
                            }
                            CallResult::Timeout => Err(ShellError::rpc_timeout()),
                            CallResult::SenderError => Err(ShellError::RpcSenderError),
                        }
                    } else {
                        Err(ShellError::NodeNotConnected(node_name.clone()))
                    }
                } else {
                    // Local query
                    if let Some(schema_json) = schema_registry::get_schema(&actor_name) {
                        self.display_schema(&actor_name, &schema_json);
                    } else {
                        println!(
                            "{} Actor '{}' does not have a registered schema.",
                            "ℹ".cyan(),
                            actor_name
                        );
                        println!(
                            "{}",
                            "  Hint: The actor may use DynamicMessage or not have registered a schema."
                                .bright_black()
                        );
                    }
                    Ok(())
                }
            }
            None => {
                // List all schema-enabled actors
                if let Some(node_name) = &self.current_node {
                    // Remote query
                    if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                        let result = introspection_ref
                            .call(
                                ShellProtocolMessage::ListSchemaActors,
                                Some(DEFAULT_RPC_TIMEOUT),
                            )
                            .await
                            .map_err(ShellError::messaging)?;

                        match result {
                            CallResult::Success(schema_actors) => {
                                if schema_actors.is_empty() {
                                    println!(
                                        "{}",
                                        "No schema-enabled actors on this node.".bright_black()
                                    );
                                } else {
                                    println!(
                                        "{} on {}:",
                                        "Schema-enabled actors".bold(),
                                        node_name.cyan()
                                    );
                                    for actor in schema_actors {
                                        println!("  {}", actor.name.green());
                                    }
                                    println!();
                                    println!(
                                        "{}",
                                        "Use 'schema <actor>' to see the message schema."
                                            .bright_black()
                                    );
                                }
                                Ok(())
                            }
                            CallResult::Timeout => Err(ShellError::rpc_timeout()),
                            CallResult::SenderError => Err(ShellError::RpcSenderError),
                        }
                    } else {
                        Err(ShellError::NodeNotConnected(node_name.clone()))
                    }
                } else {
                    // Local query
                    let schemas = schema_registry::list_schemas();
                    if schemas.is_empty() {
                        println!("{}", "No schema-enabled actors registered.".bright_black());
                        println!(
                            "{}",
                            "  Hint: Actors need to call schema_registry::register() in pre_start."
                                .bright_black()
                        );
                    } else {
                        println!("{}", "Schema-enabled actors:".bold());
                        for (name, _) in &schemas {
                            println!("  {}", name.green());
                        }
                        println!();
                        println!(
                            "{}",
                            "Use 'schema <actor>' to see the message schema.".bright_black()
                        );
                    }
                    Ok(())
                }
            }
        }
    }

    /// Display a formatted schema for an actor.
    fn display_schema(&self, actor_name: &str, schema_json: &str) {
        println!(
            "{} {}",
            "Message schema for".bold(),
            actor_name.green().bold()
        );
        println!();

        // Parse and display the schema
        if let Ok(schema) = serde_json::from_str::<serde_json::Value>(schema_json) {
            let formatted = schema_registry::format_schema(&schema);
            print!("{}", formatted);

            // Show usage example
            if let Some(variants) = schema.get("variants").and_then(|v| v.as_object()) {
                // Find a simple example variant to show
                if let Some((variant_name, _)) = variants.iter().next() {
                    println!("{}", "Example usage:".bright_black());
                    println!(
                        "  {}",
                        format!(
                            "call {} {} {{\"field\": \"value\"}}",
                            actor_name, variant_name
                        )
                        .cyan()
                    );
                }
            }
        } else {
            // Fallback: just print the raw JSON
            println!("{}", schema_json);
        }
    }

    async fn cmd_send(&self, actor: String, message: String) -> ShellResult<()> {
        println!("send {} <- {}", actor.green(), message.bright_black());
        println!();

        // Try to parse the message as JSON
        let json_value = match messages::parse_json_input(&message) {
            Ok(v) => v,
            Err(e) => {
                println!("{} {}", "Parse error:".red().bold(), e);
                println!();
                println!("{}", "Expected JSON format. Examples:".bright_black());
                println!(
                    "{}",
                    r#"  send ping_pong {{"Ping": ["shell", 1]}}"#.bright_black()
                );
                println!(
                    "{}",
                    r#"  send my_actor {{"DoWork": ["task1"]}}"#.bright_black()
                );
                return Ok(());
            }
        };

        // Check if we're in remote or local mode
        if let Some(ref node_name) = self.current_node {
            // Remote send via IntrospectionActor
            return self
                .cmd_send_remote(node_name.clone(), actor, json_value)
                .await;
        }

        // Local send attempt
        // Find the actor in registry
        if let Some(cell) = registry::where_is(actor.clone()) {
            // Try to convert to a dynamic message actor
            let dynamic_ref: ActorRef<crate::dynamic::DynamicMessage> =
                ActorRef::from(cell.clone());

            // Check if the actor supports dynamic messages
            if crate::dynamic::supports_dynamic_messages(dynamic_ref.clone()).await {
                // Actor supports dynamic messages - actually send!
                println!("{} Actor supports dynamic messages", "✓".green());

                dynamic_ref
                    .cast(crate::dynamic::DynamicMessage::Cast(json_value.clone()))
                    .map_err(ShellError::messaging)?;

                println!("{} Message sent to {}", "✓".green().bold(), actor.green());
                println!();
                println!("{}", "Message:".bold());
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_value)
                        .unwrap_or_default()
                        .bright_black()
                );
            } else {
                // Actor doesn't support dynamic messages - show educational message
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
                println!(
                    "  {} See the documentation for implementing dynamic message support.",
                    "•".bright_black()
                );
                println!(
                    "{}",
                    "  Example: use ractor_shell::dynamic::DynamicMessage;".bright_black()
                );
                println!();
                println!(
                    "{}",
                    format!("Actor: {} (ID: {})", actor.green(), cell.get_id()).bright_black()
                );
                println!(
                    "{}",
                    format!(
                        "Message: {}",
                        serde_json::to_string_pretty(&json_value).unwrap_or_default()
                    )
                    .bright_black()
                );
                println!();
                println!("{}", "How to enable:".bold());
                println!(
                    "{}",
                    "  1. Change your actor to use DynamicMessage as its Msg type".bright_black()
                );
                println!(
                    "{}",
                    "  2. Handle DynamicMessage::Cast/Call/Ping in your handler".bright_black()
                );
                println!(
                    "{}",
                    "  3. See examples/dynamic_actor.rs for a complete example".bright_black()
                );
            }
        } else {
            println!("{} Actor '{}' not found in registry", "✗".red(), actor);
        }

        Ok(())
    }

    async fn cmd_call(&self, actor: String, message: String) -> ShellResult<()> {
        println!("call {} <- {}", actor.green(), message.bright_black());
        println!();

        // Check if the message looks like typed syntax: "VariantName {args}"
        // This pattern: starts with uppercase letter, followed by optional whitespace and JSON object
        let is_typed_syntax = Self::looks_like_typed_message(&message);

        // For remote calls with typed syntax, always try typed RPC
        // (the remote node might have the schema even if we don't locally)
        if is_typed_syntax && self.current_node.is_some() {
            return self.cmd_call_typed(&actor, &message).await;
        }

        // For local calls, check if actor has a registered schema
        if schema_registry::has_schema(&actor) {
            return self.cmd_call_typed(&actor, &message).await;
        }

        // Try to parse the message as JSON for DynamicMessage actors
        let json_value = match messages::parse_json_input(&message) {
            Ok(v) => v,
            Err(e) => {
                println!("{} {}", "Parse error:".red().bold(), e);
                println!();
                println!("{}", "Expected JSON format. Examples:".bright_black());
                println!(
                    "{}",
                    r#"  call ping_pong {{"GetStats": null}}"#.bright_black()
                );
                println!(
                    "{}",
                    r#"  call my_actor {{"QueryData": ["key1"]}}"#.bright_black()
                );
                return Ok(());
            }
        };

        // Check if we're in remote or local mode
        if let Some(ref node_name) = self.current_node {
            // Remote call via IntrospectionActor
            return self
                .cmd_call_remote(node_name.clone(), actor, json_value)
                .await;
        }

        // Local call attempt
        if let Some(cell) = registry::where_is(actor.clone()) {
            // Try to convert to a dynamic message actor
            let dynamic_ref: ActorRef<crate::dynamic::DynamicMessage> =
                ActorRef::from(cell.clone());

            // Check if the actor supports dynamic messages
            if crate::dynamic::supports_dynamic_messages(dynamic_ref.clone()).await {
                // Actor supports dynamic messages - actually call!
                println!("{} Actor supports dynamic messages", "✓".green());
                println!("{} Making RPC call...", "•".bright_black());
                println!();

                // Make the RPC call with a timeout
                let result = dynamic_ref
                    .call(
                        |reply| crate::dynamic::DynamicMessage::Call(json_value.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await;

                match result {
                    Ok(CallResult::Success(response)) => match response {
                        crate::dynamic::CallResponse::Success(value) => {
                            println!("{} RPC call successful", "✓".green().bold());
                            println!();
                            println!("{}", "Response:".bold());
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&value)
                                    .unwrap_or_default()
                                    .green()
                            );
                        }
                        crate::dynamic::CallResponse::Error(err) => {
                            println!("{} RPC call failed", "✗".red().bold());
                            println!();
                            println!("{} {}", "Error:".red(), err);
                        }
                    },
                    Ok(CallResult::Timeout) => {
                        println!("{} RPC call timed out after 5 seconds", "✗".red().bold());
                    }
                    Ok(CallResult::SenderError) => {
                        println!("{} RPC call sender error", "✗".red().bold());
                    }
                    Err(e) => {
                        println!("{} RPC call error: {}", "✗".red().bold(), e);
                    }
                }
            } else {
                // Actor doesn't support dynamic messages - show educational message
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
                println!(
                    "  {} Handle DynamicMessage::Call variant to respond to RPC calls.",
                    "•".bright_black()
                );
                println!();
                println!(
                    "{}",
                    format!("Actor: {} (ID: {})", actor.green(), cell.get_id()).bright_black()
                );
                println!(
                    "{}",
                    format!(
                        "Message: {}",
                        serde_json::to_string_pretty(&json_value).unwrap_or_default()
                    )
                    .bright_black()
                );
                println!();
                println!("{}", "How to enable:".bold());
                println!(
                    "{}",
                    "  1. Change your actor to use DynamicMessage as its Msg type".bright_black()
                );
                println!(
                    "{}",
                    "  2. Handle DynamicMessage::Call and send a CallResponse".bright_black()
                );
                println!(
                    "{}",
                    "  3. See examples/dynamic_actor.rs for a complete example".bright_black()
                );
            }
        } else {
            println!("{} Actor '{}' not found in registry", "✗".red(), actor);
        }

        Ok(())
    }

    /// Call a schema-enabled actor with typed RPC
    async fn cmd_call_typed(&self, actor: &str, message: &str) -> ShellResult<()> {
        // Parse the message as "VariantName {args}" or "VariantName {}"
        let (variant, args) = Self::parse_typed_message(message)?;

        // Check if we're in remote mode
        if let Some(ref node_name) = self.current_node {
            return self
                .cmd_call_typed_remote(node_name.clone(), actor.to_string(), variant, args)
                .await;
        }

        // Local call - handle known typed actors
        let Some(_cell) = registry::where_is(actor.to_string()) else {
            println!("{} Actor '{}' not found in registry", "✗".red(), actor);
            return Ok(());
        };

        // For schema actors, show helpful message
        println!(
            "{}",
            "⚠ Typed RPC not yet implemented for this actor".yellow()
        );
        println!();
        println!(
            "  {} Actor '{}' has a schema but typed RPC requires explicit support.",
            "•".bright_black(),
            actor
        );
        println!(
            "  {} Schema: {}",
            "•".bright_black(),
            schema_registry::get_schema(actor).unwrap_or_default()
        );

        Ok(())
    }

    /// Check if a message looks like typed syntax: "VariantName {}" or "VariantName {args}"
    /// Returns true if message starts with uppercase letter and contains braces
    fn looks_like_typed_message(message: &str) -> bool {
        let message = message.trim();
        // Must start with uppercase letter (Rust enum variant convention)
        let starts_with_upper = message
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        // Must contain '{' somewhere
        let has_brace = message.contains('{');
        starts_with_upper && has_brace
    }

    /// Parse a typed message in format "VariantName {args}" or "VariantName {}"
    fn parse_typed_message(message: &str) -> ShellResult<(String, serde_json::Value)> {
        let message = message.trim();

        // Find the variant name (everything before the first '{' or whitespace)
        let variant_end = message
            .find(|c: char| c == '{' || c.is_whitespace())
            .unwrap_or(message.len());

        let variant = message[..variant_end].trim().to_string();

        if variant.is_empty() {
            return Err(ShellError::ParseError(
                "Missing variant name. Use format: VariantName {}".to_string(),
            ));
        }

        // Parse the args (everything from '{' to end, or default to {})
        let args_str = if let Some(brace_pos) = message.find('{') {
            message[brace_pos..].trim()
        } else {
            "{}"
        };

        let args: serde_json::Value = serde_json::from_str(args_str).map_err(|e| {
            ShellError::ParseError(format!(
                "Invalid JSON args: {}. Use format: VariantName {{}}",
                e
            ))
        })?;

        Ok((variant, args))
    }

    /// Call a typed RPC on a remote node
    async fn cmd_call_typed_remote(
        &self,
        node_name: String,
        actor: String,
        variant: String,
        args: serde_json::Value,
    ) -> ShellResult<()> {
        let introspection_actor = self
            .connected_nodes
            .get(&node_name)
            .ok_or_else(|| ShellError::NodeNotConnected(node_name.clone()))?;

        // Use CallTypedRpc protocol message
        let result = introspection_actor
            .call(
                |reply| {
                    ShellProtocolMessage::CallTypedRpc(
                        actor.clone(),
                        variant.clone(),
                        args.clone(),
                        reply,
                    )
                },
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        match result {
            CallResult::Success(protocol::TypedRpcResult::Success(value)) => {
                println!(
                    "{} Response from {} on {}:",
                    "✓".green().bold(),
                    actor.green(),
                    node_name.yellow()
                );
                println!();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| format!("{:?}", value))
                );
            }
            CallResult::Success(protocol::TypedRpcResult::ActorNotFound) => {
                return Err(ShellError::RemoteActorNotFound {
                    actor,
                    node: node_name,
                });
            }
            CallResult::Success(protocol::TypedRpcResult::NotSchemaEnabled) => {
                println!(
                    "{} {} is not schema-enabled on {}",
                    "!".yellow().bold(),
                    actor.yellow(),
                    node_name.yellow()
                );
            }
            CallResult::Success(protocol::TypedRpcResult::UnknownVariant(v)) => {
                println!(
                    "{} Unknown RPC variant '{}' for {}",
                    "✗".red().bold(),
                    v.red(),
                    actor.yellow()
                );
            }
            CallResult::Success(protocol::TypedRpcResult::CallFailed(err)) => {
                println!(
                    "{} Call to {} failed: {}",
                    "✗".red().bold(),
                    actor.yellow(),
                    err.red()
                );
            }
            CallResult::Timeout => {
                return Err(ShellError::rpc_timeout());
            }
            CallResult::SenderError => {
                return Err(ShellError::RpcSenderError);
            }
        }

        Ok(())
    }

    /// Send a dynamic message to an actor on a remote node
    async fn cmd_send_remote(
        &self,
        node_name: String,
        actor: String,
        json_value: serde_json::Value,
    ) -> ShellResult<()> {
        let introspection_actor = self
            .connected_nodes
            .get(&node_name)
            .ok_or_else(|| ShellError::NodeNotConnected(node_name.clone()))?;

        // Send via IntrospectionActor
        let result = introspection_actor
            .call(
                |reply| {
                    ShellProtocolMessage::SendDynamicMessage(
                        actor.clone(),
                        json_value.clone(),
                        reply,
                    )
                },
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        match result {
            CallResult::Success(protocol::DynamicSendResult::Success) => {
                println!(
                    "{} Message sent to {} on {}",
                    "✓".green().bold(),
                    actor.green(),
                    node_name.yellow()
                );
                println!();
                println!("{}", "Message:".bold());
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_value)
                        .unwrap_or_default()
                        .bright_black()
                );
            }
            CallResult::Success(protocol::DynamicSendResult::ActorNotFound) => {
                return Err(ShellError::RemoteActorNotFound {
                    actor,
                    node: node_name,
                });
            }
            CallResult::Success(protocol::DynamicSendResult::NotDynamic) => {
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
            }
            CallResult::Success(protocol::DynamicSendResult::SendFailed(err)) => {
                return Err(ShellError::RemoteOperationFailed(format!(
                    "Failed to send message: {}",
                    err
                )));
            }
            CallResult::Timeout => {
                return Err(ShellError::rpc_timeout());
            }
            CallResult::SenderError => {
                return Err(ShellError::RpcSenderError);
            }
        }

        Ok(())
    }

    /// Call an actor on a remote node with a dynamic message
    async fn cmd_call_remote(
        &self,
        node_name: String,
        actor: String,
        json_value: serde_json::Value,
    ) -> ShellResult<()> {
        let introspection_actor = self
            .connected_nodes
            .get(&node_name)
            .ok_or_else(|| ShellError::NodeNotConnected(node_name.clone()))?;

        // Call via IntrospectionActor
        let result = introspection_actor
            .call(
                |reply| {
                    ShellProtocolMessage::CallDynamicMessage(
                        actor.clone(),
                        json_value.clone(),
                        reply,
                    )
                },
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        match result {
            CallResult::Success(protocol::DynamicCallResult::Success(response)) => {
                println!(
                    "{} RPC response from {} on {}:",
                    "✓".green().bold(),
                    actor.green(),
                    node_name.yellow()
                );
                println!();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default()
                        .bright_white()
                );
            }
            CallResult::Success(protocol::DynamicCallResult::Error(err)) => {
                println!("{} Actor returned error: {}", "✗".red().bold(), err.red());
            }
            CallResult::Success(protocol::DynamicCallResult::ActorNotFound) => {
                return Err(ShellError::RemoteActorNotFound {
                    actor,
                    node: node_name,
                });
            }
            CallResult::Success(protocol::DynamicCallResult::NotDynamic) => {
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
                println!(
                    "  {} Handle DynamicMessage::Call variant to respond to RPC calls.",
                    "•".bright_black()
                );
            }
            CallResult::Success(protocol::DynamicCallResult::CallFailed(err)) => {
                return Err(ShellError::RemoteOperationFailed(format!(
                    "RPC call failed: {}",
                    err
                )));
            }
            CallResult::Timeout => {
                return Err(ShellError::rpc_timeout());
            }
            CallResult::SenderError => {
                return Err(ShellError::RpcSenderError);
            }
        }

        Ok(())
    }

    async fn cmd_send_file(&self, actor: String, file_path: String) -> ShellResult<()> {
        println!(
            "send-file {} <- {}",
            actor.green(),
            file_path.bright_black()
        );
        println!();

        // Read the file
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => {
                println!("{} Failed to read file '{}': {}", "✗".red(), file_path, e);
                return Ok(());
            }
        };

        println!(
            "{} Read {} bytes from {}",
            "✓".green(),
            content.len(),
            file_path.bright_black()
        );
        println!();

        // Use cmd_send to actually process the message
        self.cmd_send(actor, content).await
    }

    async fn cmd_load(&mut self, script_path: String) -> ShellResult<()> {
        println!("load {}", script_path.bright_black());
        println!();

        // Read the script file
        let content = match std::fs::read_to_string(&script_path) {
            Ok(c) => c,
            Err(e) => {
                println!(
                    "{} Failed to read script '{}': {}",
                    "✗".red(),
                    script_path,
                    e
                );
                return Ok(());
            }
        };

        println!(
            "{} Loaded script from {}",
            "✓".green(),
            script_path.bright_black()
        );
        println!();

        // Execute each line as a command
        let lines: Vec<&str> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#')) // Skip empty lines and comments
            .collect();

        println!(
            "{} Executing {} commands...",
            "•".bright_black(),
            lines.len()
        );
        println!();

        for (i, line) in lines.iter().enumerate() {
            println!("{} [{}] {}", "▸".bright_cyan(), i + 1, line.bright_black());

            match ShellCommand::parse_line(line) {
                Ok(cmd) => {
                    // Use Box::pin to handle recursive async calls
                    if let Err(e) = Box::pin(self.execute(cmd)).await {
                        println!("{} Command failed: {}", "✗".red(), e);
                        println!("{}", "Stopping script execution.".yellow());
                        return Ok(());
                    }
                }
                Err(e) => {
                    println!("{} Parse error: {}", "✗".red(), e);
                    println!("{}", "Stopping script execution.".yellow());
                    return Ok(());
                }
            }

            println!();
        }

        println!("{} Script execution complete", "✓".green());
        Ok(())
    }

    async fn cmd_stop(&self, actor: String) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::StopActor(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(true) => {
                        println!(
                            "{} Sent stop signal to '{}' on {}",
                            "✓".green(),
                            actor,
                            node_name
                        );
                        return Ok(());
                    }
                    CallResult::Success(false) => {
                        return Err(ShellError::RemoteActorNotFound {
                            actor,
                            node: node_name.clone(),
                        });
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution
        if let Some(cell) = registry::where_is(actor.clone()) {
            cell.stop(Some("Stopped by shell".to_string()));
            println!("{} Sent stop signal to '{}'", "✓".green(), actor);
            Ok(())
        } else {
            Err(ShellError::ActorNotFound(actor))
        }
    }

    async fn cmd_connect(&mut self, host: String) -> ShellResult<()> {
        println!("{} {}", "Connecting to".bright_black(), host.green());

        // Spawn NodeServer if not already running
        // Default port: 9100, default cookie: "secret_cookie"
        const LOCAL_NODE_SERVER_PORT: u16 = 9100;
        const LOCAL_CLUSTER_COOKIE: &str = "secret_cookie";

        if self.node_server.is_none() {
            println!("  Starting local NodeServer...");

            let node_server = ractor_cluster::NodeServer::new(
                LOCAL_NODE_SERVER_PORT,
                LOCAL_CLUSTER_COOKIE.to_string(),
                format!("{}_instance", self.local_node_name),
                self.local_node_name.clone(),
                None, // no encryption
                None, // default connection mode
            );

            let (node_ref, _node_handle) = Actor::spawn(None, node_server, ()).await?;
            self.node_server = Some(node_ref.clone());

            println!(
                "  {} NodeServer started on port {}",
                "✓".green(),
                LOCAL_NODE_SERVER_PORT
            );
        }

        // Connect to the remote node
        if let Some(ref node_ref) = self.node_server {
            ractor_cluster::client_connect(node_ref, &host)
                .await
                .map_err(|e| ShellError::ClusterError(format!("{:?}", e)))?;
            println!("  {} Connected to {}", "✓".green(), host);

            // Wait a bit for connection to establish
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Discover introspection actor in the well-known group
            println!("  Discovering introspection actor...");
            let members = pg::get_members(&INTROSPECTION_GROUP.to_string());

            // Get IDs of introspection actors we've already stored
            let known_actor_ids: std::collections::HashSet<_> = self
                .connected_nodes
                .values()
                .map(|actor_ref| actor_ref.get_id())
                .collect();

            // Find remote introspection actors we haven't seen yet
            let new_remote_introspection: Vec<ActorRef<ShellProtocolMessage>> = members
                .into_iter()
                .filter(|cell| !cell.get_id().is_local())
                .filter(|cell| !known_actor_ids.contains(&cell.get_id()))
                .map(ActorRef::<ShellProtocolMessage>::from)
                .collect();

            // If we found new actors, ping each to identify them
            // If no new actors, we're connecting to a node in an already-connected cluster
            let introspection_ref = if !new_remote_introspection.is_empty() {
                // Ping each new actor to find one that responds
                let mut found_ref = None;
                for candidate in &new_remote_introspection {
                    if let Ok(CallResult::Success(_pong)) = candidate
                        .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
                        .await
                    {
                        found_ref = Some(candidate.clone());
                        break;
                    }
                }
                found_ref.ok_or(ShellError::NoIntrospectionActor)?
            } else {
                // No new actors - this node is part of an already-connected cluster
                // All introspection actors are already known, find one we haven't stored yet
                let all_remote: Vec<ActorRef<ShellProtocolMessage>> =
                    pg::get_members(&INTROSPECTION_GROUP.to_string())
                        .into_iter()
                        .filter(|cell| !cell.get_id().is_local())
                        .filter(|cell| !known_actor_ids.contains(&cell.get_id()))
                        .map(ActorRef::<ShellProtocolMessage>::from)
                        .collect();

                // Find an actor we haven't stored yet
                let mut found_ref = None;
                for candidate in &all_remote {
                    if let Ok(CallResult::Success(_pong)) = candidate
                        .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
                        .await
                    {
                        found_ref = Some(candidate.clone());
                        break;
                    }
                }

                // If we still didn't find it, all nodes may already be connected
                found_ref.ok_or_else(|| {
                    ShellError::ClusterError(format!(
                        "Could not identify introspection actor for {}. \
                         This node may already be connected via another address.",
                        host
                    ))
                })?
            };

            // Test with a ping and display result
            let pong_result = introspection_ref
                .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
                .await
                .map_err(ShellError::messaging)?;

            match pong_result {
                CallResult::Success(pong) => {
                    println!("  {} {}", "✓".green(), pong.bright_black());
                }
                CallResult::Timeout => {
                    return Err(ShellError::rpc_timeout());
                }
                CallResult::SenderError => {
                    return Err(ShellError::RpcSenderError);
                }
            }

            // Store the connection (using host as node name for now)
            self.connected_nodes
                .insert(host.clone(), introspection_ref.clone());

            println!(
                "{} Connected to node at {}",
                "✓".green().bold(),
                host.green()
            );

            // Automatically discover cluster topology
            println!("  Discovering cluster topology...");
            let topo_result = introspection_ref
                .call(
                    ShellProtocolMessage::GetClusterTopology,
                    Some(DEFAULT_RPC_TIMEOUT),
                )
                .await
                .map_err(ShellError::messaging)?;

            match topo_result {
                CallResult::Success(topology) => {
                    let node_count = topology.nodes.len();
                    let group_count = topology.process_groups.len();
                    self.cluster_topology = Some(topology);
                    println!(
                        "  {} Discovered {} nodes and {} process groups",
                        "✓".green(),
                        node_count,
                        group_count
                    );
                }
                CallResult::Timeout => {
                    println!("  {} Topology discovery timeout (non-fatal)", "⚠".yellow());
                }
                CallResult::SenderError => {
                    println!("  {} Topology discovery failed (non-fatal)", "⚠".yellow());
                }
            }

            // Auto-switch to the connected node
            self.current_node = Some(host.clone());
            println!("{} Now using {}", "✓".green().bold(), host.yellow());
        }

        Ok(())
    }

    async fn cmd_disconnect(&mut self, node: String) -> ShellResult<()> {
        if self.connected_nodes.remove(&node).is_some() {
            println!("{} Disconnected from {}", "✓".green(), node);
            Ok(())
        } else {
            Err(ShellError::NodeNotConnected(node))
        }
    }

    async fn cmd_reconnect(&mut self, node: String) -> ShellResult<()> {
        // Remove the stale connection if it exists
        let was_current = self.current_node.as_ref() == Some(&node);
        if self.connected_nodes.remove(&node).is_some() {
            println!(
                "{} Disconnected stale connection to {}",
                "→".bright_black(),
                node
            );
        }

        // Reconnect
        self.cmd_connect(node.clone()).await?;

        // Restore as current node if it was before
        if was_current {
            self.current_node = Some(node.clone());
            println!("{} Restored {} as current node", "✓".green(), node);
        }

        Ok(())
    }

    async fn cmd_nodes(&self) -> ShellResult<()> {
        if self.connected_nodes.is_empty() {
            println!("No connected nodes");
            println!(
                "\nUse {} to connect to a remote node",
                "connect <host:port>".green()
            );
            return Ok(());
        }

        println!(
            "{}",
            format!("Connected Nodes ({}):", self.connected_nodes.len()).bold()
        );
        for node_name in self.connected_nodes.keys() {
            let indicator = if self.current_node.as_ref() == Some(node_name) {
                "*".green()
            } else {
                "•".bright_cyan()
            };
            println!("  {} {}", indicator, node_name.green());
        }

        if self.current_node.is_none() {
            println!("\nCurrently using: {}", "local".yellow());
        }

        Ok(())
    }

    async fn cmd_use(&mut self, node: String) -> ShellResult<()> {
        if node == "local" {
            self.current_node = None;
            println!("{} Switched to local node", "✓".green());
            return Ok(());
        }

        if self.connected_nodes.contains_key(&node) {
            self.current_node = Some(node.clone());
            println!("{} Switched to node {}", "✓".green(), node.green());
            Ok(())
        } else {
            Err(ShellError::NodeNotConnected(node))
        }
    }

    async fn cmd_cluster(&mut self, subcommand: Option<String>) -> ShellResult<()> {
        // We need to be connected to at least one node to query cluster topology
        if self.connected_nodes.is_empty() {
            println!(
                "{}",
                "Not connected to any nodes. Use 'connect <host:port>' first.".yellow()
            );
            return Ok(());
        }

        // Refresh cluster topology by querying any connected node
        let (_node_name, introspection_ref) = self.connected_nodes.iter().next().unwrap();

        println!("{}", "Fetching cluster topology...".bright_black());
        let result = introspection_ref
            .call(
                ShellProtocolMessage::GetClusterTopology,
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        let topology = match result {
            CallResult::Success(topo) => {
                self.cluster_topology = Some(topo.clone());
                topo
            }
            CallResult::Timeout => return Err(ShellError::rpc_timeout()),
            CallResult::SenderError => return Err(ShellError::RpcSenderError),
        };

        // Handle subcommands
        match subcommand.as_deref() {
            None | Some("nodes") => {
                // Show cluster nodes
                println!();
                println!(
                    "{}",
                    format!("Cluster Nodes ({}):", topology.nodes.len()).bold()
                );
                println!();

                #[derive(Tabled)]
                struct NodeRow {
                    #[tabled(rename = "Node ID")]
                    id: String,
                    #[tabled(rename = "Node Name")]
                    name: String,
                    #[tabled(rename = "Actors")]
                    actors: String,
                    #[tabled(rename = "Local")]
                    is_local: String,
                }

                let rows: Vec<NodeRow> = topology
                    .nodes
                    .iter()
                    .map(|node| NodeRow {
                        id: node.id.clone(),
                        name: node.name.clone(),
                        actors: node.actor_count.to_string(),
                        is_local: if node.is_local { "yes" } else { "no" }.to_string(),
                    })
                    .collect();

                let table = table::build_table(rows);
                println!("{}", table);
            }

            Some("groups") | Some("pg") => {
                // Show process groups across cluster
                println!();
                println!(
                    "{}",
                    format!("Process Groups ({}):", topology.process_groups.len()).bold()
                );
                println!();

                for (group_name, members) in &topology.process_groups {
                    println!("{}", format!("Group: {}", group_name).green().bold());

                    #[derive(Tabled)]
                    struct MemberRow {
                        #[tabled(rename = "Actor ID")]
                        id: String,
                        #[tabled(rename = "Name")]
                        name: String,
                        #[tabled(rename = "Node")]
                        node: String,
                    }

                    let rows: Vec<MemberRow> = members
                        .iter()
                        .map(|loc| MemberRow {
                            id: loc.actor_id.clone(),
                            name: loc.actor_name.clone().unwrap_or_else(|| "-".to_string()),
                            node: format!("{} ({})", loc.node_name, loc.node_id),
                        })
                        .collect();

                    let table = table::build_table(rows);
                    println!("{}", table);
                    println!();
                }

                if topology.process_groups.is_empty() {
                    println!("No process groups found in the cluster.");
                }
            }

            Some("actors") => {
                // Show all actors across cluster (from process groups)
                println!();
                println!("{}", "Actors Across Cluster:".bold());
                println!();

                let mut all_actors: Vec<_> = topology
                    .process_groups
                    .values()
                    .flat_map(|members| members.iter())
                    .collect();

                // Deduplicate by actor_id
                all_actors.sort_by(|a, b| a.actor_id.cmp(&b.actor_id));
                all_actors.dedup_by(|a, b| a.actor_id == b.actor_id);

                #[derive(Tabled)]
                struct ActorRow {
                    #[tabled(rename = "Actor ID")]
                    id: String,
                    #[tabled(rename = "Name")]
                    name: String,
                    #[tabled(rename = "Node")]
                    node: String,
                }

                let rows: Vec<ActorRow> = all_actors
                    .iter()
                    .map(|loc| ActorRow {
                        id: loc.actor_id.clone(),
                        name: loc.actor_name.clone().unwrap_or_else(|| "-".to_string()),
                        node: format!("{} ({})", loc.node_name, loc.node_id),
                    })
                    .collect();

                let table = table::build_table(rows);
                println!("{}", table);
                println!();
                println!(
                    "{}",
                    format!("Total: {} actors", all_actors.len()).bright_black()
                );
            }

            Some(unknown) => {
                println!(
                    "{} Unknown cluster subcommand: {}",
                    "Error:".red().bold(),
                    unknown
                );
                println!();
                println!("Available subcommands:");
                println!(
                    "  {} or {}  Show cluster nodes (default)",
                    "cluster".green(),
                    "cluster nodes".green()
                );
                println!("  {}       Show process groups", "cluster groups".green());
                println!("  {}       Show all actors", "cluster actors".green());
            }
        }

        Ok(())
    }

    async fn cmd_stats(&self) -> ShellResult<()> {
        println!();
        println!("{}", "System Statistics".bold().underline());
        println!();

        // Check if we're working with a remote node or local
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                println!("{} {}", "Node:".bright_black(), node_name.green());
                println!();

                // Get actor count via registry
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListRegisteredActors,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        println!("  {} {}", "Registered Actors:".bold(), actors.len());

                        // Count by status
                        let statuses: HashMap<String, usize> =
                            actors.iter().fold(HashMap::new(), |mut acc, actor| {
                                *acc.entry(actor.status.clone()).or_insert(0) += 1;
                                acc
                            });

                        for (status, count) in statuses {
                            println!("    {} {}", status.bright_black(), count);
                        }
                    }
                    _ => {
                        return Err(ShellError::RemoteOperationFailed(
                            "Failed to fetch stats".to_string(),
                        ))
                    }
                }

                println!();

                // Get cluster topology for more stats
                if let Some(ref topology) = self.cluster_topology {
                    println!("  {} {}", "Cluster Nodes:".bold(), topology.nodes.len());
                    println!(
                        "  {} {}",
                        "Process Groups:".bold(),
                        topology.process_groups.len()
                    );

                    let total_actors_in_groups: usize = topology
                        .process_groups
                        .values()
                        .map(|members| members.len())
                        .sum();
                    println!(
                        "  {} {}",
                        "Actors in Groups:".bold(),
                        total_actors_in_groups
                    );
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local stats
            println!("{} {}", "Node:".bright_black(), "local".green());
            println!();

            let registered = registry::registered();
            println!("  {} {}", "Registered Actors:".bold(), registered.len());

            // Count actors by status
            let mut status_counts: HashMap<String, usize> = HashMap::new();
            for name in &registered {
                if let Some(cell) = registry::where_is(name.clone()) {
                    let status = format!("{:?}", cell.get_status());
                    *status_counts.entry(status).or_insert(0) += 1;
                }
            }

            for (status, count) in status_counts {
                println!("    {} {}", status.bright_black(), count);
            }

            println!();

            // Process groups
            let known_groups = KNOWN_PROCESS_GROUPS;
            let mut group_count = 0;
            let mut total_members = 0;

            for group in known_groups {
                let members = pg::get_members(&group.to_string());
                if !members.is_empty() {
                    group_count += 1;
                    total_members += members.len();
                }
            }

            println!("  {} {}", "Process Groups:".bold(), group_count);
            println!("  {} {}", "Actors in Groups:".bold(), total_members);

            println!();

            // Connection info
            if !self.connected_nodes.is_empty() {
                println!(
                    "  {} {}",
                    "Connected Nodes:".bold(),
                    self.connected_nodes.len()
                );
            }

            // Cluster topology info
            if let Some(ref topology) = self.cluster_topology {
                println!(
                    "  {} {} nodes",
                    "Cluster Size:".bold(),
                    topology.nodes.len()
                );
            }
        }

        println!();
        Ok(())
    }

    /// Ping a remote node to measure latency.
    async fn cmd_ping(&mut self, node: String) -> ShellResult<()> {
        use std::time::Instant;

        // First, ensure we're connected to the node
        let introspection_ref = if let Some(existing_ref) = self.connected_nodes.get(&node) {
            existing_ref.clone()
        } else {
            // Auto-connect if not already connected
            println!("{} Connecting to {}...", "→".bright_black(), node.cyan());
            self.cmd_connect(node.clone()).await?;

            // Get the reference after connection
            self.connected_nodes
                .get(&node)
                .ok_or_else(|| ShellError::NodeNotConnected(node.clone()))?
                .clone()
        };

        // Measure round-trip time
        let start = Instant::now();
        let result = introspection_ref
            .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
            .await
            .map_err(ShellError::messaging)?;

        let elapsed = start.elapsed();

        match result {
            CallResult::Success(response) => {
                let latency_ms = elapsed.as_secs_f64() * 1000.0;
                println!("{} {} ({:.2}ms)", "✓".green(), response.green(), latency_ms);
            }
            CallResult::Timeout => {
                println!("{} Ping timeout after {:?}", "✗".red(), DEFAULT_RPC_TIMEOUT);
            }
            CallResult::SenderError => {
                println!("{} Ping failed: node unreachable", "✗".red());
            }
        }

        Ok(())
    }

    async fn cmd_tree(&self, _actor: Option<String>) -> ShellResult<()> {
        println!();
        println!("{}", "Process Group Tree".bold().underline());
        println!();

        // Helper function to display the tree
        fn display_tree(tree: &std::collections::HashMap<String, Vec<crate::protocol::ActorInfo>>) {
            if tree.is_empty() {
                println!("No process groups found");
                return;
            }

            let group_names: Vec<_> = tree.keys().collect();
            for (group_idx, group_name) in group_names.iter().enumerate() {
                let is_last_group = group_idx == group_names.len() - 1;
                let group_prefix = if is_last_group {
                    "└──"
                } else {
                    "├──"
                };
                let members = tree.get(*group_name).unwrap();

                println!(
                    "{} {} {}",
                    group_prefix.bright_cyan(),
                    group_name.green().bold(),
                    format!("[{} members]", members.len()).bright_black()
                );

                for (i, actor) in members.iter().enumerate() {
                    let is_last = i == members.len() - 1;
                    let prefix = if is_last { "└──" } else { "├──" };
                    let connector = if is_last_group { " " } else { "│" };
                    let actor_name = actor.name.clone().unwrap_or_else(|| "-".to_string());
                    let local_marker = if actor.is_local { "" } else { " (remote)" };
                    println!(
                        "{}   {} {} {}{}",
                        connector.bright_cyan(),
                        prefix.bright_cyan(),
                        actor_name.cyan(),
                        format!("({})", actor.id).bright_black(),
                        local_marker.bright_black()
                    );
                }
                println!();
            }
        }

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::GetProcessGroupTree,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(tree) => {
                        println!(
                            "{}",
                            format!("Process groups on {} ({} total):", node_name, tree.len())
                                .bright_black()
                        );
                        println!();
                        display_tree(&tree);
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
                return Ok(());
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution - use pg::which_groups() for dynamic group discovery
        let groups = pg::which_groups();
        let mut tree: std::collections::HashMap<String, Vec<crate::protocol::ActorInfo>> =
            std::collections::HashMap::new();

        for group in groups {
            let members = pg::get_members(&group);
            let actors: Vec<crate::protocol::ActorInfo> = members
                .iter()
                .map(crate::protocol::ActorInfo::from_cell)
                .collect();
            tree.insert(group, actors);
        }

        println!(
            "{}",
            format!("Process groups ({} total):", tree.len()).bright_black()
        );
        println!();
        display_tree(&tree);

        println!(
            "{}",
            "Tip: Use 'supervtree' to see actor supervision hierarchy".bright_black()
        );
        println!();

        Ok(())
    }

    async fn cmd_supervtree(&self, actor: Option<String>) -> ShellResult<()> {
        use ractor::ActorCell;

        println!();
        println!("{}", "Supervision Tree".bold().underline());
        println!();

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetSupervisionTree(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(tree_nodes) => {
                        if tree_nodes.is_empty() {
                            if let Some(ref name) = actor {
                                return Err(ShellError::RemoteActorNotFound {
                                    actor: name.clone(),
                                    node: node_name.clone(),
                                });
                            } else {
                                println!(
                                    "{}",
                                    "No root actors found on remote node.".bright_black()
                                );
                            }
                        } else {
                            for (i, tree_node) in tree_nodes.iter().enumerate() {
                                let is_last = i == tree_nodes.len() - 1;
                                self.print_supervision_subtree_remote(tree_node, "", is_last);
                                if !is_last {
                                    println!();
                                }
                            }
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            if let Some(actor_name) = actor {
                // Show tree starting from specific actor
                if let Some(cell) = registry::where_is(actor_name.clone()) {
                    self.print_supervision_subtree(&cell, "", true);
                } else {
                    return Err(ShellError::ActorNotFound(actor_name));
                }
            } else {
                // Find and display all root actors (those without supervisors)
                let all_actors = self.get_all_local_actors();
                let roots: Vec<ActorCell> = all_actors
                    .iter()
                    .filter(|cell| cell.try_get_supervisor().is_none())
                    .cloned()
                    .collect();

                if roots.is_empty() {
                    println!("{}", "No root actors found.".bright_black());
                } else {
                    for (i, root) in roots.iter().enumerate() {
                        let is_last = i == roots.len() - 1;
                        self.print_supervision_subtree(root, "", is_last);
                        if !is_last {
                            println!();
                        }
                    }
                }
            }
        }

        println!();
        Ok(())
    }

    async fn cmd_parent(&self, actor: String) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetActorParent(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(parent_info) => {
                        println!("{}", format!("Actor on {}: {}", node_name, actor).bold());
                        match parent_info {
                            Some(supervisor) => {
                                let supervisor_name =
                                    supervisor.name.unwrap_or_else(|| supervisor.id.to_string());
                                println!(
                                    "  Supervisor: {} {}",
                                    supervisor_name.green(),
                                    format!("({})", supervisor.id).bright_black()
                                );
                                println!("  Status:     {}", supervisor.status);
                            }
                            None => {
                                println!(
                                    "  {}",
                                    "No supervisor (this is a root actor)".bright_black()
                                );
                            }
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            if let Some(cell) = registry::where_is(actor.clone()) {
                match cell.try_get_supervisor() {
                    Some(supervisor) => {
                        println!("{}", format!("Actor: {}", actor).bold());
                        println!(
                            "  Supervisor: {} {}",
                            supervisor
                                .get_name()
                                .unwrap_or_else(|| "<unnamed>".to_string())
                                .green(),
                            format!("({})", supervisor.get_id()).bright_black()
                        );
                        println!("  Status:     {:?}", supervisor.get_status());
                    }
                    None => {
                        println!("{}", format!("Actor: {}", actor).bold());
                        println!(
                            "  {}",
                            "No supervisor (this is a root actor)".bright_black()
                        );
                    }
                }
            } else {
                return Err(ShellError::ActorNotFound(actor));
            }
        }
        Ok(())
    }

    /// Helper function to get all local actors from registry and process groups.
    fn get_all_local_actors(&self) -> Vec<ractor::ActorCell> {
        use std::collections::HashSet;

        let mut actors = Vec::new();
        let mut seen_ids = HashSet::new();

        // Add registered actors
        for name in registry::registered() {
            if let Some(cell) = registry::where_is(name) {
                let id = cell.get_id();
                if seen_ids.insert(id) {
                    actors.push(cell);
                }
            }
        }

        // Add actors from known process groups
        for group in KNOWN_PROCESS_GROUPS {
            for cell in pg::get_members(&group.to_string()) {
                let id = cell.get_id();
                if seen_ids.insert(id) {
                    actors.push(cell);
                }
            }
        }

        actors
    }

    /// Recursively print a supervision tree starting from the given actor.
    fn print_supervision_subtree(&self, cell: &ractor::ActorCell, prefix: &str, is_last: bool) {
        let actor_name = cell.get_name().unwrap_or_else(|| "<unnamed>".to_string());
        let actor_id = cell.get_id().to_string();
        let status = format!("{:?}", cell.get_status());
        let children = cell.get_children();
        let child_count = children.len();

        // Print current actor
        let branch = if is_last { "└──" } else { "├──" };
        let name_part = if child_count > 0 {
            format!("{} [{}]", actor_name, child_count).green().bold()
        } else {
            actor_name.cyan()
        };

        println!(
            "{}{} {} {} {}",
            prefix.bright_cyan(),
            branch.bright_cyan(),
            name_part,
            format!("({})", actor_id).bright_black(),
            format!("[{}]", status).bright_black()
        );

        // Print children recursively
        if !children.is_empty() {
            let child_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };

            for (i, child) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;
                self.print_supervision_subtree(child, &child_prefix, is_last_child);
            }
        }
    }

    /// Recursively print a supervision tree from remote SupervisionTreeNode data.
    fn print_supervision_subtree_remote(
        &self,
        node: &protocol::SupervisionTreeNode,
        prefix: &str,
        is_last: bool,
    ) {
        let actor_name = node
            .actor
            .name
            .clone()
            .unwrap_or_else(|| "<unnamed>".to_string());
        let actor_id = &node.actor.id;
        let status = &node.actor.status;
        let child_count = node.child_count;

        // Print current actor
        let branch = if is_last { "└──" } else { "├──" };
        let name_part = if child_count > 0 {
            format!("{} [{}]", actor_name, child_count).green().bold()
        } else {
            actor_name.cyan()
        };

        println!(
            "{}{} {} {} {}",
            prefix.bright_cyan(),
            branch.bright_cyan(),
            name_part,
            format!("({})", actor_id).bright_black(),
            format!("[{}]", status).bright_black()
        );

        // Print children recursively
        if !node.children.is_empty() {
            let child_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };

            for (i, child) in node.children.iter().enumerate() {
                let is_last_child = i == node.children.len() - 1;
                self.print_supervision_subtree_remote(child, &child_prefix, is_last_child);
            }
        }
    }

    async fn cmd_monitor(&self, actor: String) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::StartMonitoring(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(true) => {
                        println!(
                            "{} Now monitoring '{}' on {}",
                            "✓".green(),
                            actor,
                            node_name
                        );
                        return Ok(());
                    }
                    CallResult::Success(false) => {
                        return Err(ShellError::RemoteActorNotFound {
                            actor,
                            node: node_name.clone(),
                        });
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution
        if let Some(monitor_ref) = &self.monitor_actor {
            monitor_ref
                .cast(monitor::MonitorMessage::Monitor { actor_name: actor })
                .map_err(ShellError::messaging)?;
        } else {
            println!("{} Monitor system not available", "✗".red());
        }
        Ok(())
    }

    async fn cmd_unmonitor(&self, actor: String) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::StopMonitoring(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(true) => {
                        println!(
                            "{} Stopped monitoring '{}' on {}",
                            "✓".green(),
                            actor,
                            node_name
                        );
                        return Ok(());
                    }
                    CallResult::Success(false) => {
                        println!(
                            "{} Actor '{}' was not being monitored on {}",
                            "!".yellow(),
                            actor,
                            node_name
                        );
                        return Ok(());
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution
        if let Some(monitor_ref) = &self.monitor_actor {
            monitor_ref
                .cast(monitor::MonitorMessage::Unmonitor { actor_name: actor })
                .map_err(ShellError::messaging)?;
        } else {
            println!("{} Monitor system not available", "✗".red());
        }
        Ok(())
    }

    async fn cmd_monitors(&self) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::GetMonitoredActors,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(monitored) => {
                        if monitored.is_empty() {
                            println!(
                                "{}",
                                format!("No actors currently being monitored on {}", node_name)
                                    .bright_black()
                            );
                        } else {
                            println!(
                                "{}",
                                format!("Monitored Actors on {}:", node_name)
                                    .bold()
                                    .underline()
                            );
                            for actor_name in monitored {
                                println!("  {} {}", "•".bright_cyan(), actor_name.cyan());
                            }
                        }
                        return Ok(());
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution
        if let Some(monitor_ref) = &self.monitor_actor {
            match ractor::call!(monitor_ref, monitor::MonitorMessage::GetMonitored) {
                Ok(monitored) => {
                    if monitored.is_empty() {
                        println!("{}", "No actors currently being monitored".bright_black());
                    } else {
                        println!("{}", "Monitored Actors:".bold().underline());
                        for actor_name in monitored {
                            println!("  {} {}", "•".bright_cyan(), actor_name.cyan());
                        }
                    }
                }
                Err(e) => {
                    println!("{} Failed to get monitored actors: {}", "✗".red(), e);
                }
            }
        } else {
            println!("{} Monitor system not available", "✗".red());
        }
        Ok(())
    }

    async fn cmd_monitor_events(&self) -> ShellResult<()> {
        // This command only works when connected to a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::PollMonitorEvents,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(batch) => {
                        if batch.events.is_empty() && batch.dropped_count == 0 {
                            println!(
                                "{}",
                                format!("No new monitor events on {}", node_name).bright_black()
                            );
                        } else {
                            // Display events
                            for event in &batch.events {
                                let level_str = &event.event_type;
                                let level_colored = match level_str.as_str() {
                                    "STARTED" => format!("▲ {}", level_str).green(),
                                    "STOPPED" => format!("▼ {}", level_str).yellow(),
                                    "STOPPING" => format!("◊ {}", level_str).yellow(),
                                    "PANICKED" => format!("✗ {}", level_str).red(),
                                    "KILLED" => format!("⊗ {}", level_str).red(),
                                    _ => level_str.white(),
                                };

                                let actor_info = if let Some(name) = &event.actor_name {
                                    format!("{} ({})", name, event.actor_id)
                                } else {
                                    event.actor_id.clone()
                                };

                                println!(
                                    "[{}] {} {} {}",
                                    event.timestamp.bright_black(),
                                    level_colored,
                                    actor_info.cyan(),
                                    event
                                        .reason
                                        .as_ref()
                                        .or(event.error.as_ref())
                                        .map(|s| format!("- {}", s))
                                        .unwrap_or_default()
                                        .bright_black()
                                );
                            }

                            // Warn if events were dropped
                            if batch.dropped_count > 0 {
                                println!(
                                    "{} [{}] {} monitor events dropped due to buffer overflow",
                                    "⚠".yellow(),
                                    node_name.yellow(),
                                    batch.dropped_count
                                );
                            }
                        }
                        return Ok(());
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local context - events are displayed automatically by MonitorActor
        println!(
            "{} {} {}",
            "Note:".yellow(),
            "Local monitor events are displayed automatically.".bright_black(),
            "Use 'monitor events' only when connected to a remote node.".bright_black()
        );
        Ok(())
    }
}

/// Shell commands that can be executed in the REPL.
///
/// Commands are parsed from user input via [`ShellCommand::parse_line`] and
/// executed via [`ShellState::execute`].
#[derive(Debug, Clone)]
pub enum ShellCommand {
    /// Display help information. Alias: `help [command]`
    Help { command: Option<String> },
    /// Exit the shell. Aliases: `exit`, `quit`, `q`
    Exit,
    /// List all actors (registered and in process groups). Alias: `a`
    Actors,
    /// List registered actors only. Alias: `r`
    Registry,
    /// List all process groups. Usage: `pg list`
    PgList,
    /// List members of a process group. Usage: `pg members <group>`
    PgMembers { group: String },
    /// Show detailed information about an actor. Alias: `i`
    Info { actor: String },
    /// Send a cast message to an actor. Alias: `s`. Usage: `send <actor> <json>`
    Send { actor: String, message: String },
    /// Send an RPC call to an actor. Alias: `c`. Usage: `call <actor> <json>`
    Call { actor: String, message: String },
    /// Stop an actor gracefully. Usage: `stop <actor>`
    Stop { actor: String },
    /// Connect to a remote node. Usage: `connect <host:port>`
    Connect { host: String },
    /// Disconnect from a remote node. Usage: `disconnect <node>`
    Disconnect { node: String },
    /// Reconnect to a remote node (disconnect + connect). Usage: `reconnect <node>`
    Reconnect { node: String },
    /// List connected nodes
    Nodes,
    /// Switch context to a remote node. Usage: `use <node>` or `use local`
    Use { node: String },
    /// Show cluster topology. Usage: `cluster [nodes|groups|mesh]`
    Cluster { subcommand: Option<String> },
    /// Show system statistics
    Stats,
    /// Show process group tree (limited in Phase 1)
    Tree { actor: Option<String> },
    /// Show actual supervision tree with parent-child relationships. Alias: `st`. Usage: `supervtree [actor]`
    Supervtree { actor: Option<String> },
    /// Show the supervisor (parent) of an actor. Alias: `p`. Usage: `parent <actor>`
    Parent { actor: String },
    /// Send a JSON file as a message. Alias: `sf`. Usage: `send-file <actor> <path>`
    SendFile { actor: String, file_path: String },
    /// Load and execute a script file. Alias: `l`. Usage: `load <path>`
    Load { script_path: String },
    /// Start monitoring an actor's lifecycle. Usage: `monitor <actor>`
    Monitor { actor: String },
    /// Stop monitoring an actor. Usage: `unmonitor <actor>`
    Unmonitor { actor: String },
    /// List all monitored actors and recent events
    Monitors,
    /// Poll and display monitor events (remote only). Usage: `monitor events`
    MonitorEvents,
    /// Launch interactive TUI dashboard. Alias: `t`. Usage: `top`
    Top,
    /// Start tracing actors or list active traces. Usage: `trace [pattern]`
    Trace { pattern: Option<String> },
    /// Stop all tracing. Usage: `trace off`
    TraceOff,
    /// Get or set minimum trace log level. Usage: `trace level [LEVEL]`
    TraceLevel { level: Option<String> },
    /// Log traces to a file. Usage: `trace-to-file <path> [pattern]`
    TraceToFile {
        path: String,
        pattern: Option<String>,
    },
    /// Subscribe to remote traces from a node. Alias: `tr`. Usage: `trace remote <node> <pattern>`
    TraceRemote { node: String, pattern: String },
    /// Stop remote tracing. Usage: `trace remote off`
    TraceRemoteOff,
    /// Show message schema for an actor. Alias: `sc`. Usage: `schema <actor>` or `schema` to list all
    Schema { actor: Option<String> },
    /// Ping a remote node to measure latency. Usage: `ping <node>`
    Ping { node: String },
}

impl ShellCommand {
    /// Resolve command aliases
    fn resolve_alias(cmd: &str) -> &str {
        match cmd {
            "a" => "actors",
            "r" => "registry",
            "i" => "info",
            "s" => "send",
            "c" => "call",
            "sf" => "send-file",
            "l" => "load",
            "q" => "quit",
            "t" => "top",
            "tr" => "trace",
            "tf" => "trace-to-file",
            "sc" => "schema",
            "st" => "supervtree",
            "p" => "parent",
            _ => cmd,
        }
    }

    /// Parse a command line string into a ShellCommand.
    ///
    /// Supports command aliases (e.g., `a` for `actors`, `q` for `quit`).
    /// Returns an error if the command is not recognized or missing required arguments.
    pub fn parse_line(line: &str) -> ShellResult<Self> {
        let mut parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            return Err(ShellError::EmptyCommand);
        }

        // Resolve aliases
        let resolved_cmd = Self::resolve_alias(parts[0]);
        parts[0] = resolved_cmd;

        match parts[0] {
            "help" => Ok(ShellCommand::Help {
                command: parts.get(1).map(|s| s.to_string()),
            }),
            "exit" | "quit" => Ok(ShellCommand::Exit),
            "actors" => Ok(ShellCommand::Actors),
            "registry" => Ok(ShellCommand::Registry),
            "pg" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "pg",
                        requirement: "a subcommand: list, members",
                    });
                }
                match parts[1] {
                    "list" => Ok(ShellCommand::PgList),
                    "members" => {
                        if parts.len() < 3 {
                            return Err(ShellError::MissingArgument {
                                command: "pg members",
                                requirement: "a group name",
                            });
                        }
                        Ok(ShellCommand::PgMembers {
                            group: parts[2].to_string(),
                        })
                    }
                    _ => Err(ShellError::UnknownSubcommand {
                        parent: "pg",
                        subcommand: parts[1].to_string(),
                    }),
                }
            }
            "info" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "info",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Info {
                    actor: parts[1].to_string(),
                })
            }
            "send" => {
                if parts.len() < 3 {
                    return Err(ShellError::MissingArgument {
                        command: "send",
                        requirement: "<actor> <message>",
                    });
                }
                Ok(ShellCommand::Send {
                    actor: parts[1].to_string(),
                    message: parts[2..].join(" "),
                })
            }
            "call" => {
                if parts.len() < 3 {
                    return Err(ShellError::MissingArgument {
                        command: "call",
                        requirement: "<actor> <message>",
                    });
                }
                Ok(ShellCommand::Call {
                    actor: parts[1].to_string(),
                    message: parts[2..].join(" "),
                })
            }
            "stop" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "stop",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Stop {
                    actor: parts[1].to_string(),
                })
            }
            "connect" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "connect",
                        requirement: "<host:port>",
                    });
                }
                Ok(ShellCommand::Connect {
                    host: parts[1].to_string(),
                })
            }
            "disconnect" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "disconnect",
                        requirement: "<node_name>",
                    });
                }
                Ok(ShellCommand::Disconnect {
                    node: parts[1].to_string(),
                })
            }
            "reconnect" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "reconnect",
                        requirement: "<node_name>",
                    });
                }
                Ok(ShellCommand::Reconnect {
                    node: parts[1].to_string(),
                })
            }
            "nodes" => Ok(ShellCommand::Nodes),
            "use" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "use",
                        requirement: "<node_name> (or 'local')",
                    });
                }
                Ok(ShellCommand::Use {
                    node: parts[1].to_string(),
                })
            }
            "cluster" => Ok(ShellCommand::Cluster {
                subcommand: parts.get(1).map(|s| s.to_string()),
            }),
            "stats" => Ok(ShellCommand::Stats),
            "tree" => Ok(ShellCommand::Tree {
                actor: parts.get(1).map(|s| s.to_string()),
            }),
            "supervtree" => Ok(ShellCommand::Supervtree {
                actor: parts.get(1).map(|s| s.to_string()),
            }),
            "parent" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "parent",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Parent {
                    actor: parts[1].to_string(),
                })
            }
            "send-file" | "sendfile" => {
                if parts.len() < 3 {
                    return Err(ShellError::MissingArgument {
                        command: "send-file",
                        requirement: "<actor> <file_path>",
                    });
                }
                Ok(ShellCommand::SendFile {
                    actor: parts[1].to_string(),
                    file_path: parts[2].to_string(),
                })
            }
            "load" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "load",
                        requirement: "<script_path>",
                    });
                }
                Ok(ShellCommand::Load {
                    script_path: parts[1].to_string(),
                })
            }
            "monitor" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "monitor",
                        requirement: "an actor name or 'events'",
                    });
                }
                // Check for subcommands
                if parts[1] == "events" {
                    return Ok(ShellCommand::MonitorEvents);
                }
                Ok(ShellCommand::Monitor {
                    actor: parts[1].to_string(),
                })
            }
            "unmonitor" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "unmonitor",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Unmonitor {
                    actor: parts[1].to_string(),
                })
            }
            "monitors" => Ok(ShellCommand::Monitors),
            "top" => Ok(ShellCommand::Top),
            "trace" => {
                match parts.get(1).copied() {
                    Some("off") => Ok(ShellCommand::TraceOff),
                    Some("level") => {
                        // "trace level" or "trace level <LEVEL>"
                        Ok(ShellCommand::TraceLevel {
                            level: parts.get(2).map(|s| s.to_string()),
                        })
                    }
                    Some("remote") => {
                        // "trace remote off" or "trace remote <node> <pattern>"
                        if parts.get(2).copied() == Some("off") {
                            Ok(ShellCommand::TraceRemoteOff)
                        } else if parts.len() < 4 {
                            Err(ShellError::MissingArgument {
                                command: "trace remote",
                                requirement: "<node> <pattern> or 'off'",
                            })
                        } else {
                            Ok(ShellCommand::TraceRemote {
                                node: parts[2].to_string(),
                                pattern: parts[3].to_string(),
                            })
                        }
                    }
                    _ => Ok(ShellCommand::Trace {
                        pattern: parts.get(1).map(|s| s.to_string()),
                    }),
                }
            }
            "trace-to-file" | "tracefile" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "trace-to-file",
                        requirement: "<file_path> [pattern]",
                    });
                }
                Ok(ShellCommand::TraceToFile {
                    path: parts[1].to_string(),
                    pattern: parts.get(2).map(|s| s.to_string()),
                })
            }
            "schema" => Ok(ShellCommand::Schema {
                actor: parts.get(1).map(|s| s.to_string()),
            }),
            "ping" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "ping",
                        requirement: "<node> (e.g., 127.0.0.1:9001)",
                    });
                }
                Ok(ShellCommand::Ping {
                    node: parts[1].to_string(),
                })
            }
            _ => Err(ShellError::UnknownCommand(parts[0].to_string())),
        }
    }
}

#[cfg(test)]
mod lib_tests;

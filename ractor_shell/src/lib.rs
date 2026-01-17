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
//! - [`dynamic`]: Dynamic message interface for shell-to-actor communication
//! - [`introspection`]: Remote introspection actor for cross-node queries
//! - [`monitor`]: Actor lifecycle monitoring
//! - [`protocol`]: Shell protocol messages for cluster communication
//! - [`completer`]: Tab completion support

use anyhow::{anyhow, Result};
use colored::Colorize;
use ractor::{rpc::CallResult, Actor, ActorRef};
use std::collections::HashMap;
use tabled::{Table, Tabled};

pub mod commands;
pub mod completer;
pub mod dynamic;
pub mod introspection;
pub mod messages;
pub mod monitor;
pub mod protocol;

use introspection::INTROSPECTION_GROUP;
use protocol::{ClusterTopology, ShellProtocolMessage};

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
}

impl ShellState {
    /// Create a new shell state with default configuration.
    ///
    /// This spawns the monitor actor and initializes the local node name
    /// based on the system hostname.
    pub async fn new() -> Result<Self> {
        let local_node_name = format!(
            "shell@{}",
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "localhost".to_string())
        );

        // Spawn monitor actor
        let (monitor_ref, _) =
            Actor::spawn(Some("shell_monitor".to_string()), monitor::MonitorActor, ()).await?;

        Ok(Self {
            should_exit: false,
            current_node: None,
            local_node_name,
            node_server: None,
            connected_nodes: HashMap::new(),
            cluster_topology: None,
            monitor_actor: Some(monitor_ref),
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
    pub async fn execute(&mut self, cmd: ShellCommand) -> Result<()> {
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
            ShellCommand::Nodes => self.cmd_nodes().await,
            ShellCommand::Use { node } => self.cmd_use(node).await,
            ShellCommand::Cluster { subcommand } => self.cmd_cluster(subcommand).await,
            ShellCommand::Stats => self.cmd_stats().await,
            ShellCommand::Tree { actor } => self.cmd_tree(actor).await,
            ShellCommand::SendFile { actor, file_path } => {
                self.cmd_send_file(actor, file_path).await
            }
            ShellCommand::Load { script_path } => self.cmd_load(script_path).await,
            ShellCommand::Monitor { actor } => self.cmd_monitor(actor).await,
            ShellCommand::Unmonitor { actor } => self.cmd_unmonitor(actor).await,
            ShellCommand::Monitors => self.cmd_monitors().await,
        }
    }

    async fn cmd_help(&self, command: Option<String>) -> Result<()> {
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
                    println!("  monitor <actor_name>");
                    println!("\n{}", "Example:".bold());
                    println!("  monitor demo_actor_1");
                    println!("\n{}", "Note:".bold());
                    println!("  - Shows start, stop, panic, and kill events");
                    println!("  - Events are displayed in real-time");
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
                "  {} Aliases: {}, {}, {}, {}, {}, {}",
                "•".bright_cyan(),
                "a=actors".bright_black(),
                "r=registry".bright_black(),
                "i=info".bright_black(),
                "s=send".bright_black(),
                "c=call".bright_black(),
                "q=quit".bright_black()
            );
            println!();
            println!("Type 'help <command>' for more information on a specific command");
        }
        Ok(())
    }

    async fn cmd_exit(&mut self) -> Result<()> {
        self.should_exit = true;
        Ok(())
    }

    async fn cmd_actors(&self) -> Result<()> {
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
                        Some(tokio::time::Duration::from_secs(5)),
                    )
                    .await?;

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

                        let table = Table::new(rows).to_string();
                        println!("{}", table);
                    }
                    CallResult::Timeout => return Err(anyhow!("Request timeout")),
                    CallResult::SenderError => return Err(anyhow!("Request sender error")),
                }
            } else {
                return Err(anyhow!("Not connected to node '{}'", node_name));
            }
        } else {
            // Local query
            println!("{}", "Listing actors...".bright_black());
            println!("Note: Phase 1 only shows named actors. Full enumeration coming in Phase 2.");

            let registered = ractor::registry::registered();

            if registered.is_empty() {
                println!("No registered actors found.");
                return Ok(());
            }

            let mut rows = Vec::new();
            for name in registered {
                if let Some(cell) = ractor::registry::where_is(name.clone()) {
                    rows.push(ActorRow {
                        name: name.clone(),
                        id: cell.get_id().to_string(),
                        status: format!("{:?}", cell.get_status()),
                    });
                }
            }

            let table = Table::new(rows).to_string();
            println!("{}", table);
        }

        Ok(())
    }

    async fn cmd_registry(&self) -> Result<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListRegisteredActors,
                        Some(tokio::time::Duration::from_secs(5)),
                    )
                    .await?;

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
                    CallResult::Timeout => return Err(anyhow!("Request timeout")),
                    CallResult::SenderError => return Err(anyhow!("Request sender error")),
                }
            } else {
                return Err(anyhow!("Not connected to node '{}'", node_name));
            }
        } else {
            // Local registry query
            let registered = ractor::registry::registered();

            if registered.is_empty() {
                println!("No registered actors.");
                return Ok(());
            }

            println!(
                "{}",
                format!("Registered Actors ({}):", registered.len()).bold()
            );
            for name in registered {
                if let Some(cell) = ractor::registry::where_is(name.clone()) {
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

    async fn cmd_pg_list(&self) -> Result<()> {
        println!("{}", "Process Groups:".bold());
        println!("Note: Process group enumeration not available in ractor 0.15");
        println!("You can query specific groups with: pg members <group_name>");
        println!();
        println!("Known groups:");
        println!("  - {}", "ping_pong".green());
        println!("  - {}", "ractor_shell_introspection".green());
        Ok(())
    }

    async fn cmd_pg_members(&self, group: String) -> Result<()> {
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
                        Some(tokio::time::Duration::from_secs(5)),
                    )
                    .await?;

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

                        let table = Table::new(rows).to_string();
                        println!("{}", table);
                    }
                    CallResult::Timeout => return Err(anyhow!("Request timeout")),
                    CallResult::SenderError => return Err(anyhow!("Request sender error")),
                }
            } else {
                return Err(anyhow!("Not connected to node '{}'", node_name));
            }
        } else {
            // Local query
            let members = ractor::pg::get_members(&group);

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

            let table = Table::new(rows).to_string();
            println!("{}", table);
        }

        Ok(())
    }

    async fn cmd_info(&self, actor: String) -> Result<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetActorInfo(actor.clone(), reply),
                        Some(tokio::time::Duration::from_secs(5)),
                    )
                    .await?;

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
                    CallResult::Success(None) => Err(anyhow!(
                        "Actor '{}' not found on node '{}'",
                        actor,
                        node_name
                    )),
                    CallResult::Timeout => Err(anyhow!("Request timeout")),
                    CallResult::SenderError => Err(anyhow!("Request sender error")),
                }
            } else {
                Err(anyhow!("Not connected to node '{}'", node_name))
            }
        } else {
            // Local query
            if let Some(cell) = ractor::registry::where_is(actor.clone()) {
                println!("{}", format!("Actor: {}", actor).bold());
                println!("  ID:     {}", cell.get_id());
                println!("  Status: {:?}", cell.get_status());
                if let Some(name) = cell.get_name() {
                    println!("  Name:   {}", name);
                }
                Ok(())
            } else {
                Err(anyhow!("Actor '{}' not found in registry", actor))
            }
        }
    }

    async fn cmd_send(&self, actor: String, message: String) -> Result<()> {
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
        if let Some(_node_name) = &self.current_node {
            // Remote send - not yet implemented
            println!("{}", "Remote message sending not yet implemented.".yellow());
            println!("{}", "Use 'use local' to switch to local mode.".yellow());
            return Ok(());
        }

        // Local send attempt
        // Find the actor in registry
        if let Some(cell) = ractor::registry::where_is(actor.clone()) {
            // Try to convert to a dynamic message actor
            let dynamic_ref: ActorRef<crate::dynamic::DynamicMessage> =
                ActorRef::from(cell.clone());

            // Check if the actor supports dynamic messages
            if crate::dynamic::supports_dynamic_messages(dynamic_ref.clone()).await {
                // Actor supports dynamic messages - actually send!
                println!("{} Actor supports dynamic messages", "✓".green());

                dynamic_ref.cast(crate::dynamic::DynamicMessage::Cast(json_value.clone()))?;

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

    async fn cmd_call(&self, actor: String, message: String) -> Result<()> {
        println!("call {} <- {}", actor.green(), message.bright_black());
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
        if let Some(_node_name) = &self.current_node {
            // Remote call - not yet implemented
            println!("{}", "Remote RPC calls not yet implemented.".yellow());
            println!("{}", "Use 'use local' to switch to local mode.".yellow());
            return Ok(());
        }

        // Local call attempt
        if let Some(cell) = ractor::registry::where_is(actor.clone()) {
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
                        Some(tokio::time::Duration::from_secs(5)),
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

    async fn cmd_send_file(&self, actor: String, file_path: String) -> Result<()> {
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

    async fn cmd_load(&mut self, script_path: String) -> Result<()> {
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

    async fn cmd_stop(&self, actor: String) -> Result<()> {
        if let Some(cell) = ractor::registry::where_is(actor.clone()) {
            cell.stop(Some("Stopped by shell".to_string()));
            println!("{} Sent stop signal to '{}'", "✓".green(), actor);
            Ok(())
        } else {
            Err(anyhow!("Actor '{}' not found in registry", actor))
        }
    }

    async fn cmd_connect(&mut self, host: String) -> Result<()> {
        println!("{} {}", "Connecting to".bright_black(), host.green());

        // Spawn NodeServer if not already running
        if self.node_server.is_none() {
            println!("  Starting local NodeServer...");

            const DEFAULT_PORT: u16 = 9100;
            const COOKIE: &str = "secret_cookie";

            let node_server = ractor_cluster::NodeServer::new(
                DEFAULT_PORT,
                COOKIE.to_string(),
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
                DEFAULT_PORT
            );
        }

        // Connect to the remote node
        if let Some(ref node_ref) = self.node_server {
            ractor_cluster::client_connect(node_ref, &host).await?;
            println!("  {} Connected to {}", "✓".green(), host);

            // Wait a bit for connection to establish
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Discover introspection actor in the well-known group
            println!("  Discovering introspection actor...");
            let members = ractor::pg::get_members(&INTROSPECTION_GROUP.to_string());

            let remote_introspection: Vec<ActorRef<ShellProtocolMessage>> = members
                .into_iter()
                .filter(|cell| !cell.get_id().is_local())
                .map(ActorRef::<ShellProtocolMessage>::from)
                .collect();

            if remote_introspection.is_empty() {
                return Err(anyhow!(
                    "No introspection actor found on remote node. \
                     Make sure the target node has spawned an IntrospectionActor."
                ));
            }

            // For now, take the first remote introspection actor
            // In a full implementation, we'd track which actor belongs to which node
            let introspection_ref = remote_introspection[0].clone();

            // Test with a ping
            let pong_result = introspection_ref
                .call(
                    ShellProtocolMessage::Ping,
                    Some(tokio::time::Duration::from_secs(5)),
                )
                .await?;

            match pong_result {
                CallResult::Success(pong) => {
                    println!("  {} {}", "✓".green(), pong.bright_black());
                }
                CallResult::Timeout => {
                    return Err(anyhow!("Ping timeout"));
                }
                CallResult::SenderError => {
                    return Err(anyhow!("Ping sender error"));
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
                    Some(tokio::time::Duration::from_secs(5)),
                )
                .await?;

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
        }

        Ok(())
    }

    async fn cmd_disconnect(&mut self, node: String) -> Result<()> {
        if self.connected_nodes.remove(&node).is_some() {
            println!("{} Disconnected from {}", "✓".green(), node);
            Ok(())
        } else {
            Err(anyhow!("Not connected to node '{}'", node))
        }
    }

    async fn cmd_nodes(&self) -> Result<()> {
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

    async fn cmd_use(&mut self, node: String) -> Result<()> {
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
            Err(anyhow!(
                "Not connected to node '{}'. Use 'nodes' to see connected nodes.",
                node
            ))
        }
    }

    async fn cmd_cluster(&mut self, subcommand: Option<String>) -> Result<()> {
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
                Some(tokio::time::Duration::from_secs(5)),
            )
            .await?;

        let topology = match result {
            CallResult::Success(topo) => {
                self.cluster_topology = Some(topo.clone());
                topo
            }
            CallResult::Timeout => return Err(anyhow!("Topology query timeout")),
            CallResult::SenderError => return Err(anyhow!("Topology query sender error")),
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

                let table = Table::new(rows).to_string();
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

                    let table = Table::new(rows).to_string();
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

                let table = Table::new(rows).to_string();
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

    async fn cmd_stats(&self) -> Result<()> {
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
                        Some(tokio::time::Duration::from_secs(5)),
                    )
                    .await?;

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
                    _ => return Err(anyhow!("Failed to fetch stats from remote node")),
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
                return Err(anyhow!("Not connected to node '{}'", node_name));
            }
        } else {
            // Local stats
            println!("{} {}", "Node:".bright_black(), "local".green());
            println!();

            let registered = ractor::registry::registered();
            println!("  {} {}", "Registered Actors:".bold(), registered.len());

            // Count actors by status
            let mut status_counts: HashMap<String, usize> = HashMap::new();
            for name in &registered {
                if let Some(cell) = ractor::registry::where_is(name.clone()) {
                    let status = format!("{:?}", cell.get_status());
                    *status_counts.entry(status).or_insert(0) += 1;
                }
            }

            for (status, count) in status_counts {
                println!("    {} {}", status.bright_black(), count);
            }

            println!();

            // Process groups
            let known_groups = vec!["ping_pong", "ractor_shell_introspection", "demo_group"];
            let mut group_count = 0;
            let mut total_members = 0;

            for group in known_groups {
                let members = ractor::pg::get_members(&group.to_string());
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

    async fn cmd_tree(&self, _actor: Option<String>) -> Result<()> {
        println!();
        println!("{}", "Process Group Tree".bold().underline());
        println!();
        println!(
            "{}",
            "Note: Full supervision tree introspection requires ractor core API enhancements."
                .yellow()
        );
        println!(
            "{}",
            "      Showing process group organization as a simplified tree.".yellow()
        );
        println!();

        // For now, show process groups as a simple tree structure
        // In the future, this would show actual parent/child supervision relationships

        if self.connected_nodes.is_empty() {
            // Local tree
            let known_groups = vec!["ping_pong", "ractor_shell_introspection", "demo_group"];

            for group in known_groups {
                let members = ractor::pg::get_members(&group.to_string());
                if !members.is_empty() {
                    println!("{} {}", "├──".bright_cyan(), group.green().bold());
                    for (i, cell) in members.iter().enumerate() {
                        let is_last = i == members.len() - 1;
                        let prefix = if is_last { "└──" } else { "├──" };
                        let actor_name = cell.get_name().unwrap_or_else(|| "-".to_string());
                        let actor_id = cell.get_id().to_string();
                        println!(
                            "{}   {} {} {}",
                            "│".bright_cyan(),
                            prefix.bright_cyan(),
                            actor_name.cyan(),
                            format!("({})", actor_id).bright_black()
                        );
                    }
                    println!();
                }
            }
        } else {
            // Use cluster topology if available
            if let Some(ref topology) = self.cluster_topology {
                for (group_name, members) in &topology.process_groups {
                    println!("{} {}", "├──".bright_cyan(), group_name.green().bold());
                    for (i, member) in members.iter().enumerate() {
                        let is_last = i == members.len() - 1;
                        let prefix = if is_last { "└──" } else { "├──" };
                        let actor_name =
                            member.actor_name.clone().unwrap_or_else(|| "-".to_string());
                        let node_info = format!("on {}", member.node_name);
                        println!(
                            "{}   {} {} {} {}",
                            "│".bright_cyan(),
                            prefix.bright_cyan(),
                            actor_name.cyan(),
                            format!("({})", member.actor_id).bright_black(),
                            node_info.bright_black()
                        );
                    }
                    println!();
                }
            } else {
                println!("No cluster topology available. Connect to a node first.");
            }
        }

        println!();
        println!("{}", "Future Enhancement:".bright_black());
        println!(
            "{}",
            "  Once ractor exposes supervision tree APIs, this command will show:".bright_black()
        );
        println!(
            "{}",
            "  - True parent/child actor relationships".bright_black()
        );
        println!("{}", "  - Supervisor strategies".bright_black());
        println!("{}", "  - Actor restart counts".bright_black());
        println!();

        Ok(())
    }

    async fn cmd_monitor(&self, actor: String) -> Result<()> {
        if let Some(monitor_ref) = &self.monitor_actor {
            monitor_ref.cast(monitor::MonitorMessage::Monitor { actor_name: actor })?;
        } else {
            println!("{} Monitor system not available", "✗".red());
        }
        Ok(())
    }

    async fn cmd_unmonitor(&self, actor: String) -> Result<()> {
        if let Some(monitor_ref) = &self.monitor_actor {
            monitor_ref.cast(monitor::MonitorMessage::Unmonitor { actor_name: actor })?;
        } else {
            println!("{} Monitor system not available", "✗".red());
        }
        Ok(())
    }

    async fn cmd_monitors(&self) -> Result<()> {
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
    /// List connected nodes
    Nodes,
    /// Switch context to a remote node. Usage: `use <node>` or `use local`
    Use { node: String },
    /// Show cluster topology. Usage: `cluster [nodes|groups|mesh]`
    Cluster { subcommand: Option<String> },
    /// Show system statistics
    Stats,
    /// Show supervision tree (limited in Phase 1)
    Tree { actor: Option<String> },
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
            _ => cmd,
        }
    }

    /// Parse a command line string into a ShellCommand.
    ///
    /// Supports command aliases (e.g., `a` for `actors`, `q` for `quit`).
    /// Returns an error if the command is not recognized or missing required arguments.
    pub fn parse_line(line: &str) -> Result<Self> {
        let mut parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            return Err(anyhow!("Empty command"));
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
                    return Err(anyhow!("pg requires a subcommand: list, members"));
                }
                match parts[1] {
                    "list" => Ok(ShellCommand::PgList),
                    "members" => {
                        if parts.len() < 3 {
                            return Err(anyhow!("pg members requires a group name"));
                        }
                        Ok(ShellCommand::PgMembers {
                            group: parts[2].to_string(),
                        })
                    }
                    _ => Err(anyhow!("Unknown pg subcommand: {}", parts[1])),
                }
            }
            "info" => {
                if parts.len() < 2 {
                    return Err(anyhow!("info requires an actor name"));
                }
                Ok(ShellCommand::Info {
                    actor: parts[1].to_string(),
                })
            }
            "send" => {
                if parts.len() < 3 {
                    return Err(anyhow!("send requires: send <actor> <message>"));
                }
                Ok(ShellCommand::Send {
                    actor: parts[1].to_string(),
                    message: parts[2..].join(" "),
                })
            }
            "call" => {
                if parts.len() < 3 {
                    return Err(anyhow!("call requires: call <actor> <message>"));
                }
                Ok(ShellCommand::Call {
                    actor: parts[1].to_string(),
                    message: parts[2..].join(" "),
                })
            }
            "stop" => {
                if parts.len() < 2 {
                    return Err(anyhow!("stop requires an actor name"));
                }
                Ok(ShellCommand::Stop {
                    actor: parts[1].to_string(),
                })
            }
            "connect" => {
                if parts.len() < 2 {
                    return Err(anyhow!("connect requires: connect <host:port>"));
                }
                Ok(ShellCommand::Connect {
                    host: parts[1].to_string(),
                })
            }
            "disconnect" => {
                if parts.len() < 2 {
                    return Err(anyhow!("disconnect requires: disconnect <node_name>"));
                }
                Ok(ShellCommand::Disconnect {
                    node: parts[1].to_string(),
                })
            }
            "nodes" => Ok(ShellCommand::Nodes),
            "use" => {
                if parts.len() < 2 {
                    return Err(anyhow!("use requires: use <node_name> (or 'local')"));
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
            "send-file" | "sendfile" => {
                if parts.len() < 3 {
                    return Err(anyhow!("send-file requires: send-file <actor> <file_path>"));
                }
                Ok(ShellCommand::SendFile {
                    actor: parts[1].to_string(),
                    file_path: parts[2].to_string(),
                })
            }
            "load" => {
                if parts.len() < 2 {
                    return Err(anyhow!("load requires: load <script_path>"));
                }
                Ok(ShellCommand::Load {
                    script_path: parts[1].to_string(),
                })
            }
            "monitor" => {
                if parts.len() < 2 {
                    return Err(anyhow!("monitor requires an actor name"));
                }
                Ok(ShellCommand::Monitor {
                    actor: parts[1].to_string(),
                })
            }
            "unmonitor" => {
                if parts.len() < 2 {
                    return Err(anyhow!("unmonitor requires an actor name"));
                }
                Ok(ShellCommand::Unmonitor {
                    actor: parts[1].to_string(),
                })
            }
            "monitors" => Ok(ShellCommand::Monitors),
            _ => Err(anyhow!("Unknown command: {}", parts[0])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Command Parsing Tests ====================

    #[test]
    fn test_parse_empty_line() {
        let result = ShellCommand::parse_line("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty command"));
    }

    #[test]
    fn test_parse_whitespace_only() {
        let result = ShellCommand::parse_line("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = ShellCommand::parse_line("foobar");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

    // Basic commands without arguments
    #[test]
    fn test_parse_actors() {
        let cmd = ShellCommand::parse_line("actors").unwrap();
        assert!(matches!(cmd, ShellCommand::Actors));
    }

    #[test]
    fn test_parse_registry() {
        let cmd = ShellCommand::parse_line("registry").unwrap();
        assert!(matches!(cmd, ShellCommand::Registry));
    }

    #[test]
    fn test_parse_nodes() {
        let cmd = ShellCommand::parse_line("nodes").unwrap();
        assert!(matches!(cmd, ShellCommand::Nodes));
    }

    #[test]
    fn test_parse_stats() {
        let cmd = ShellCommand::parse_line("stats").unwrap();
        assert!(matches!(cmd, ShellCommand::Stats));
    }

    #[test]
    fn test_parse_monitors() {
        let cmd = ShellCommand::parse_line("monitors").unwrap();
        assert!(matches!(cmd, ShellCommand::Monitors));
    }

    #[test]
    fn test_parse_exit() {
        let cmd = ShellCommand::parse_line("exit").unwrap();
        assert!(matches!(cmd, ShellCommand::Exit));
    }

    #[test]
    fn test_parse_quit() {
        let cmd = ShellCommand::parse_line("quit").unwrap();
        assert!(matches!(cmd, ShellCommand::Exit));
    }

    // Commands with arguments
    #[test]
    fn test_parse_help_no_arg() {
        let cmd = ShellCommand::parse_line("help").unwrap();
        match cmd {
            ShellCommand::Help { command } => assert!(command.is_none()),
            _ => panic!("Expected Help command"),
        }
    }

    #[test]
    fn test_parse_help_with_arg() {
        let cmd = ShellCommand::parse_line("help actors").unwrap();
        match cmd {
            ShellCommand::Help { command } => assert_eq!(command, Some("actors".to_string())),
            _ => panic!("Expected Help command"),
        }
    }

    #[test]
    fn test_parse_info() {
        let cmd = ShellCommand::parse_line("info my_actor").unwrap();
        match cmd {
            ShellCommand::Info { actor } => assert_eq!(actor, "my_actor"),
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_parse_info_missing_arg() {
        let result = ShellCommand::parse_line("info");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires an actor name"));
    }

    #[test]
    fn test_parse_stop() {
        let cmd = ShellCommand::parse_line("stop my_actor").unwrap();
        match cmd {
            ShellCommand::Stop { actor } => assert_eq!(actor, "my_actor"),
            _ => panic!("Expected Stop command"),
        }
    }

    #[test]
    fn test_parse_send() {
        let cmd = ShellCommand::parse_line(r#"send my_actor {"cmd": "ping"}"#).unwrap();
        match cmd {
            ShellCommand::Send { actor, message } => {
                assert_eq!(actor, "my_actor");
                assert_eq!(message, r#"{"cmd": "ping"}"#);
            }
            _ => panic!("Expected Send command"),
        }
    }

    #[test]
    fn test_parse_send_multiword_message() {
        let cmd = ShellCommand::parse_line("send actor hello world").unwrap();
        match cmd {
            ShellCommand::Send { actor, message } => {
                assert_eq!(actor, "actor");
                assert_eq!(message, "hello world");
            }
            _ => panic!("Expected Send command"),
        }
    }

    #[test]
    fn test_parse_send_missing_args() {
        let result = ShellCommand::parse_line("send actor");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_call() {
        let cmd = ShellCommand::parse_line(r#"call my_actor {"cmd": "get"}"#).unwrap();
        match cmd {
            ShellCommand::Call { actor, message } => {
                assert_eq!(actor, "my_actor");
                assert_eq!(message, r#"{"cmd": "get"}"#);
            }
            _ => panic!("Expected Call command"),
        }
    }

    #[test]
    fn test_parse_connect() {
        let cmd = ShellCommand::parse_line("connect localhost:9000").unwrap();
        match cmd {
            ShellCommand::Connect { host } => assert_eq!(host, "localhost:9000"),
            _ => panic!("Expected Connect command"),
        }
    }

    #[test]
    fn test_parse_disconnect() {
        let cmd = ShellCommand::parse_line("disconnect node_a").unwrap();
        match cmd {
            ShellCommand::Disconnect { node } => assert_eq!(node, "node_a"),
            _ => panic!("Expected Disconnect command"),
        }
    }

    #[test]
    fn test_parse_use() {
        let cmd = ShellCommand::parse_line("use node_b").unwrap();
        match cmd {
            ShellCommand::Use { node } => assert_eq!(node, "node_b"),
            _ => panic!("Expected Use command"),
        }
    }

    #[test]
    fn test_parse_use_local() {
        let cmd = ShellCommand::parse_line("use local").unwrap();
        match cmd {
            ShellCommand::Use { node } => assert_eq!(node, "local"),
            _ => panic!("Expected Use command"),
        }
    }

    // pg subcommands
    #[test]
    fn test_parse_pg_list() {
        let cmd = ShellCommand::parse_line("pg list").unwrap();
        assert!(matches!(cmd, ShellCommand::PgList));
    }

    #[test]
    fn test_parse_pg_members() {
        let cmd = ShellCommand::parse_line("pg members my_group").unwrap();
        match cmd {
            ShellCommand::PgMembers { group } => assert_eq!(group, "my_group"),
            _ => panic!("Expected PgMembers command"),
        }
    }

    #[test]
    fn test_parse_pg_missing_subcommand() {
        let result = ShellCommand::parse_line("pg");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a subcommand"));
    }

    #[test]
    fn test_parse_pg_members_missing_group() {
        let result = ShellCommand::parse_line("pg members");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a group name"));
    }

    #[test]
    fn test_parse_pg_unknown_subcommand() {
        let result = ShellCommand::parse_line("pg foobar");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown pg subcommand"));
    }

    // cluster command
    #[test]
    fn test_parse_cluster_no_subcommand() {
        let cmd = ShellCommand::parse_line("cluster").unwrap();
        match cmd {
            ShellCommand::Cluster { subcommand } => assert!(subcommand.is_none()),
            _ => panic!("Expected Cluster command"),
        }
    }

    #[test]
    fn test_parse_cluster_with_subcommand() {
        let cmd = ShellCommand::parse_line("cluster nodes").unwrap();
        match cmd {
            ShellCommand::Cluster { subcommand } => {
                assert_eq!(subcommand, Some("nodes".to_string()))
            }
            _ => panic!("Expected Cluster command"),
        }
    }

    // tree command
    #[test]
    fn test_parse_tree_no_arg() {
        let cmd = ShellCommand::parse_line("tree").unwrap();
        match cmd {
            ShellCommand::Tree { actor } => assert!(actor.is_none()),
            _ => panic!("Expected Tree command"),
        }
    }

    #[test]
    fn test_parse_tree_with_actor() {
        let cmd = ShellCommand::parse_line("tree my_actor").unwrap();
        match cmd {
            ShellCommand::Tree { actor } => assert_eq!(actor, Some("my_actor".to_string())),
            _ => panic!("Expected Tree command"),
        }
    }

    // File commands
    #[test]
    fn test_parse_send_file() {
        let cmd = ShellCommand::parse_line("send-file actor /path/to/file.json").unwrap();
        match cmd {
            ShellCommand::SendFile { actor, file_path } => {
                assert_eq!(actor, "actor");
                assert_eq!(file_path, "/path/to/file.json");
            }
            _ => panic!("Expected SendFile command"),
        }
    }

    #[test]
    fn test_parse_sendfile_alt() {
        let cmd = ShellCommand::parse_line("sendfile actor file.json").unwrap();
        match cmd {
            ShellCommand::SendFile { actor, file_path } => {
                assert_eq!(actor, "actor");
                assert_eq!(file_path, "file.json");
            }
            _ => panic!("Expected SendFile command"),
        }
    }

    #[test]
    fn test_parse_load() {
        let cmd = ShellCommand::parse_line("load /path/to/script.sh").unwrap();
        match cmd {
            ShellCommand::Load { script_path } => assert_eq!(script_path, "/path/to/script.sh"),
            _ => panic!("Expected Load command"),
        }
    }

    // Monitor commands
    #[test]
    fn test_parse_monitor() {
        let cmd = ShellCommand::parse_line("monitor my_actor").unwrap();
        match cmd {
            ShellCommand::Monitor { actor } => assert_eq!(actor, "my_actor"),
            _ => panic!("Expected Monitor command"),
        }
    }

    #[test]
    fn test_parse_unmonitor() {
        let cmd = ShellCommand::parse_line("unmonitor my_actor").unwrap();
        match cmd {
            ShellCommand::Unmonitor { actor } => assert_eq!(actor, "my_actor"),
            _ => panic!("Expected Unmonitor command"),
        }
    }

    // ==================== Alias Tests ====================

    #[test]
    fn test_alias_a_for_actors() {
        let cmd = ShellCommand::parse_line("a").unwrap();
        assert!(matches!(cmd, ShellCommand::Actors));
    }

    #[test]
    fn test_alias_r_for_registry() {
        let cmd = ShellCommand::parse_line("r").unwrap();
        assert!(matches!(cmd, ShellCommand::Registry));
    }

    #[test]
    fn test_alias_i_for_info() {
        let cmd = ShellCommand::parse_line("i my_actor").unwrap();
        match cmd {
            ShellCommand::Info { actor } => assert_eq!(actor, "my_actor"),
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_alias_s_for_send() {
        let cmd = ShellCommand::parse_line("s actor message").unwrap();
        match cmd {
            ShellCommand::Send { actor, message } => {
                assert_eq!(actor, "actor");
                assert_eq!(message, "message");
            }
            _ => panic!("Expected Send command"),
        }
    }

    #[test]
    fn test_alias_c_for_call() {
        let cmd = ShellCommand::parse_line("c actor message").unwrap();
        match cmd {
            ShellCommand::Call { actor, message } => {
                assert_eq!(actor, "actor");
                assert_eq!(message, "message");
            }
            _ => panic!("Expected Call command"),
        }
    }

    #[test]
    fn test_alias_sf_for_send_file() {
        let cmd = ShellCommand::parse_line("sf actor file.json").unwrap();
        match cmd {
            ShellCommand::SendFile { actor, file_path } => {
                assert_eq!(actor, "actor");
                assert_eq!(file_path, "file.json");
            }
            _ => panic!("Expected SendFile command"),
        }
    }

    #[test]
    fn test_alias_l_for_load() {
        let cmd = ShellCommand::parse_line("l script.sh").unwrap();
        match cmd {
            ShellCommand::Load { script_path } => assert_eq!(script_path, "script.sh"),
            _ => panic!("Expected Load command"),
        }
    }

    #[test]
    fn test_alias_q_for_quit() {
        let cmd = ShellCommand::parse_line("q").unwrap();
        assert!(matches!(cmd, ShellCommand::Exit));
    }
}

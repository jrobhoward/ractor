//! Monitoring Commands
//!
//! Commands for monitoring actor lifecycle events:
//! - `monitor` - Start monitoring an actor
//! - `unmonitor` - Stop monitoring an actor
//! - `monitors` - List monitored actors
//! - `monitor events` - Poll and display monitor events (remote only)

use colored::Colorize;
use ractor::rpc::CallResult;

use crate::error::{ShellError, ShellResult};
use crate::monitor;
use crate::protocol::ShellProtocolMessage;
use crate::ShellState;
use crate::DEFAULT_RPC_TIMEOUT;

impl ShellState {
    /// Start monitoring an actor's lifecycle events.
    pub(crate) async fn cmd_monitor(&self, actor: String) -> ShellResult<()> {
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

    /// Stop monitoring an actor.
    pub(crate) async fn cmd_unmonitor(&self, actor: String) -> ShellResult<()> {
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

    /// List all monitored actors.
    pub(crate) async fn cmd_monitors(&self) -> ShellResult<()> {
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

    /// Poll and display monitor events (remote only).
    pub(crate) async fn cmd_monitor_events(&self) -> ShellResult<()> {
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

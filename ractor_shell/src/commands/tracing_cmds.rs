//! Tracing Commands
//!
//! Commands for tracing actor behavior:
//! - `trace` - Start tracing actors or list active traces
//! - `trace off` - Stop all tracing
//! - `trace level` - Get or set minimum trace log level
//! - `trace-to-file` - Log traces to a file
//! - `trace remote` - Subscribe to remote traces
//! - `trace remote off` - Stop remote tracing

use std::time::Duration;

use colored::Colorize;
use ractor::rpc::CallResult;

use crate::error::{ShellError, ShellResult};
use crate::protocol::ShellProtocolMessage;
use crate::tracing::{self, MinLevel};
use crate::ShellState;
use crate::{RemoteTraceSubscription, DEFAULT_RPC_TIMEOUT};

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

impl ShellState {
    /// Start tracing actors or list active traces.
    pub(crate) async fn cmd_trace(&mut self, pattern: Option<String>) -> ShellResult<()> {
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
    pub(crate) async fn cmd_trace_off(&mut self) -> ShellResult<()> {
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
    pub(crate) async fn cmd_trace_level(&mut self, level: Option<String>) -> ShellResult<()> {
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
    pub(crate) async fn cmd_trace_to_file(
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
    pub(crate) async fn cmd_trace_remote(
        &mut self,
        node: String,
        pattern: String,
    ) -> ShellResult<()> {
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
    pub(crate) async fn cmd_trace_remote_off(&mut self) -> ShellResult<()> {
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
}

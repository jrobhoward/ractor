//! Utility Commands
//!
//! Miscellaneous commands:
//! - `exit` - Exit the shell
//! - `stop` - Stop an actor
//! - `load` - Execute shell commands from a script file
//! - `top` - Launch interactive TUI dashboard

use colored::Colorize;
use ractor::registry;
use ractor::rpc::CallResult;

use crate::commands::ShellCommand;
use crate::error::{ShellError, ShellResult};
use crate::protocol::ShellProtocolMessage;
use crate::ShellState;
use crate::{tui, DEFAULT_RPC_TIMEOUT};

impl ShellState {
    /// Exit the shell.
    pub(crate) async fn cmd_exit(&mut self) -> ShellResult<()> {
        self.should_exit = true;
        Ok(())
    }

    /// Stop an actor gracefully.
    pub(crate) async fn cmd_stop(&self, actor: String) -> ShellResult<()> {
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

    /// Execute shell commands from a script file.
    pub(crate) async fn cmd_load(&mut self, script_path: String) -> ShellResult<()> {
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

    /// Launch the interactive TUI dashboard.
    pub(crate) async fn cmd_top(&mut self) -> ShellResult<()> {
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
}

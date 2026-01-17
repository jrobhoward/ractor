//! Monitoring example for ractor shell
//!
//! This example demonstrates actor lifecycle monitoring features:
//! - Monitoring actor start/stop events
//! - Handling actor panics
//! - Tracking monitored actors
//!
//! To run:
//! ```
//! cargo run --example monitoring_demo
//! ```
//!
//! Try these commands:
//! ```
//! monitor demo_actor_1    # Start monitoring
//! monitors                # List monitored actors
//! stop demo_actor_1       # See a stop event
//! unmonitor demo_actor_2  # Stop monitoring
//! ```

use anyhow::Result;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use ractor_shell::completer::{get_known_process_groups, update_completer_state, ShellHelper};
use ractor_shell::{ShellCommand, ShellState};
use rustyline::Editor;
use serde::{Deserialize, Serialize};

/// Simple demo actor that can handle basic messages
pub struct DemoActor {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DemoMessage {
    Ping,
}

impl Actor for DemoActor {
    type Msg = DemoMessage;
    type State = ();
    type Arguments = String;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        name: String,
    ) -> Result<Self::State, ActorProcessingErr> {
        println!("✓ {} started", name);
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DemoMessage::Ping => {
                println!("  {} received Ping", self.name);
            }
        }
        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        println!("✓ {} stopped", self.name);
        Ok(())
    }
}

/// Actor that will panic when it receives a specific message
pub struct PanickyActor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PanickyMessage {
    Panic,
    Normal,
}

impl Actor for PanickyActor {
    type Msg = PanickyMessage;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            PanickyMessage::Panic => {
                panic!("Intentional panic for monitoring demo");
            }
            PanickyMessage::Normal => {
                println!("  panicky_actor received Normal message");
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Ractor Shell - Monitoring Demo");
    println!();
    println!("This demo shows actor lifecycle monitoring.");
    println!("Actors will start and you can monitor their lifecycle events.");
    println!();

    // Create shell state
    let mut state = ShellState::new().await?;

    // Spawn some demo actors
    println!("Spawning demo actors...");
    for i in 1..=3 {
        let name = format!("demo_actor_{}", i);
        let actor = DemoActor { name: name.clone() };

        let (_actor_ref, _handle) = Actor::spawn(Some(name.clone()), actor, name.clone()).await?;
    }

    // Spawn a panicky actor
    let (_panicky_ref, _panicky_handle) =
        Actor::spawn(Some("panicky_actor".to_string()), PanickyActor, ()).await?;

    println!();
    println!("Available commands:");
    println!("  monitor <actor>     - Start monitoring an actor");
    println!("  unmonitor <actor>   - Stop monitoring an actor");
    println!("  monitors            - List monitored actors");
    println!("  stop <actor>        - Stop an actor (see stop event)");
    println!("  registry            - Show all actors");
    println!("  help                - Show all commands");
    println!();
    println!("Try:");
    println!("  monitor demo_actor_1");
    println!("  monitors");
    println!("  stop demo_actor_1");
    println!();

    // Setup rustyline with tab completion
    let helper = ShellHelper::new();
    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));

    // Load history
    let history_file = dirs::data_local_dir()
        .map(|d| d.join("ractor_shell").join("history.txt"))
        .unwrap_or_else(|| std::path::PathBuf::from(".ractor_shell_history"));

    if let Some(parent) = history_file.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let _ = rl.load_history(&history_file);

    loop {
        // Update completer state before each prompt
        if let Some(helper) = rl.helper_mut() {
            let actor_names = ractor::registry::registered();
            let process_groups = get_known_process_groups();
            let node_names = state.connected_nodes.keys().cloned().collect();
            update_completer_state(helper, actor_names, process_groups, node_names);
        }

        let prompt = state.build_prompt();
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line)?;

                match ShellCommand::parse_line(line) {
                    Ok(cmd) => {
                        if let Err(e) = state.execute(cmd).await {
                            println!("❌ Error: {}", e);
                        }

                        if state.should_exit {
                            break;
                        }
                    }
                    Err(e) => {
                        println!("❌ {}", e);
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C");
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&history_file);

    println!("Goodbye!");
    Ok(())
}

use colored::Colorize;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use ractor_shell::completer::{ShellHelper, get_known_process_groups, update_completer_state};
use ractor_shell::dynamic::{CallResponse, DynamicMessage};
use ractor_shell::{ShellCommand, ShellState};
use rustyline::Editor;
use serde_json::json;

/// Example actor that supports dynamic JSON messages from the shell
struct DynamicActor;

/// Actor state
struct DynamicActorState {
    counter: i32,
    status: String,
}

impl Actor for DynamicActor {
    type Msg = DynamicMessage;
    type State = DynamicActorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        println!("  [DynamicActor] Started");
        Ok(DynamicActorState {
            counter: 0,
            status: "idle".to_string(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DynamicMessage::Cast(json) => {
                println!("  [DynamicActor] Received cast: {}", json);

                // Handle different message patterns
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "increment" => {
                            state.counter += 1;
                            println!("  [DynamicActor] Counter incremented to {}", state.counter);
                        }
                        "reset" => {
                            state.counter = 0;
                            println!("  [DynamicActor] Counter reset");
                        }
                        "set_status" => {
                            if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
                                state.status = status.to_string();
                                println!("  [DynamicActor] Status set to: {}", status);
                            }
                        }
                        _ => {
                            println!("  [DynamicActor] Unknown command: {}", cmd);
                        }
                    }
                } else {
                    println!("  [DynamicActor] Received unstructured message");
                }
                Ok(())
            }
            DynamicMessage::Call(json, reply) => {
                println!("  [DynamicActor] Received call: {}", json);

                // Handle RPC messages
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "get_counter" => {
                            let response = json!({
                                "counter": state.counter
                            });
                            let _ = reply.send(CallResponse::Success(response));
                        }
                        "get_status" => {
                            let response = json!({
                                "status": state.status,
                                "counter": state.counter
                            });
                            let _ = reply.send(CallResponse::Success(response));
                        }
                        "add" => {
                            if let Some(amount) = json.get("amount").and_then(|v| v.as_i64()) {
                                state.counter += amount as i32;
                                let response = json!({
                                    "new_value": state.counter
                                });
                                let _ = reply.send(CallResponse::Success(response));
                            } else {
                                let _ = reply.send(CallResponse::Error(
                                    "Missing or invalid 'amount' field".to_string(),
                                ));
                            }
                        }
                        _ => {
                            let _ = reply
                                .send(CallResponse::Error(format!("Unknown command: {}", cmd)));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error(
                        "No 'command' field in message".to_string(),
                    ));
                }
                Ok(())
            }
            DynamicMessage::Ping(reply) => {
                let _ = reply.send(true);
                Ok(())
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!(
        "{}",
        "Ractor Shell - Dynamic Actor Demo".bright_cyan().bold()
    );
    println!();
    println!("This demo shows actors that can receive JSON messages from the shell.");
    println!();

    // Spawn a dynamic actor
    let (actor_ref, _) = Actor::spawn(Some("dynamic_actor".to_string()), DynamicActor, ()).await?;

    println!("✓ Spawned dynamic_actor");
    println!();

    // Join to process group
    ractor::pg::join("dynamic_group".to_string(), vec![actor_ref.get_cell()]);

    println!("{}", "Try these commands:".bold());
    println!();
    println!("{}", "  Cast messages (fire-and-forget):".bright_black());
    println!(
        "  {}",
        r#"send dynamic_actor {"command": "increment"}"#.green()
    );
    println!("  {}", r#"send dynamic_actor {"command": "reset"}"#.green());
    println!(
        "  {}",
        r#"send dynamic_actor {"command": "set_status", "status": "running"}"#.green()
    );
    println!();
    println!("{}", "  RPC messages (request-reply):".bright_black());
    println!(
        "  {}",
        r#"call dynamic_actor {"command": "get_counter"}"#.green()
    );
    println!(
        "  {}",
        r#"call dynamic_actor {"command": "get_status"}"#.green()
    );
    println!(
        "  {}",
        r#"call dynamic_actor {"command": "add", "amount": 5}"#.green()
    );
    println!();
    println!("{}", "  Other commands:".bright_black());
    println!(
        "  {}             - See actor info",
        "info dynamic_actor".green()
    );
    println!("  {}          - Show registered actors", "registry".green());
    println!("  {}               - Exit the shell", "exit".green());
    println!();

    // Initialize shell
    let mut state = ShellState::new().await?;

    // Create editor with tab completion
    let helper = ShellHelper::new();
    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));

    // REPL loop
    loop {
        // Update completer with current state
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

                let _ = rl.add_history_entry(line);

                match ShellCommand::parse_line(line) {
                    Ok(cmd) => {
                        if let Err(e) = state.execute(cmd).await {
                            eprintln!("{} {}", "Error:".red().bold(), e);
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Parse error:".red().bold(), e);
                    }
                }

                if state.should_exit {
                    break;
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    println!("\n{}", "Goodbye!".bright_cyan());
    Ok(())
}

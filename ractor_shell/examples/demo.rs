use colored::Colorize;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use ractor_shell::completer::{get_known_process_groups, update_completer_state, ShellHelper};
use ractor_shell::{ShellCommand, ShellState};
use rustyline::Editor;

// Simple demo actor
struct DemoActor;

#[derive(Debug)]
#[allow(dead_code)]
enum DemoMessage {
    Ping,
}

impl ractor::Message for DemoMessage {}

impl Actor for DemoActor {
    type Msg = DemoMessage;
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
        _message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        println!("  [DemoActor] Received ping!");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{}", "Ractor Shell Demo".bright_cyan().bold());
    println!("Spawning some demo actors...\n");

    // Spawn some named actors
    let (actor1, _) = Actor::spawn(Some("demo_actor_1".to_string()), DemoActor, ()).await?;

    let (actor2, _) = Actor::spawn(Some("demo_actor_2".to_string()), DemoActor, ()).await?;

    let (actor3, _) = Actor::spawn(Some("demo_actor_3".to_string()), DemoActor, ()).await?;

    // Join them to a process group
    ractor::pg::join(
        "demo_group".to_string(),
        vec![actor1.get_cell(), actor2.get_cell(), actor3.get_cell()],
    );

    println!("✓ Spawned 3 actors:");
    println!("  - demo_actor_1");
    println!("  - demo_actor_2");
    println!("  - demo_actor_3");
    println!("\n✓ Joined them to process group: demo_group");
    println!("\nNow starting the shell...\n");
    println!("Try these commands:");
    println!("  {}        - See the actors", "registry".green());
    println!("  {}          - List all actors", "actors".green());
    println!(
        "  {}  - Show process group members",
        "pg members demo_group".green()
    );
    println!(
        "  {}  - See details about an actor",
        "info demo_actor_1".green()
    );
    println!("\n");

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

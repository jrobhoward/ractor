use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use ractor_shell::{ShellCommand, ShellState};

#[derive(Parser)]
#[command(name = "ractor-shell")]
#[command(about = "Interactive shell for Ractor actor systems", long_about = None)]
struct Cli {
    /// Connect to a remote node on startup
    #[arg(short, long)]
    connect: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("{}", "Ractor Shell v0.1.0".bright_cyan().bold());
    println!("Type 'help' for available commands, 'exit' to quit\n");

    // Initialize shell state
    let mut state = ShellState::new().await?;

    // Auto-connect if specified
    if let Some(addr) = cli.connect {
        println!("Connecting to {}...", addr);
        // TODO: implement auto-connect
    }

    // Set up readline editor with history
    let mut rl = DefaultEditor::new()?;
    let history_path = dirs::home_dir().map(|mut p| {
        p.push(".ractor_shell_history");
        p
    });

    // Load history if it exists
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    // Main REPL loop
    loop {
        let prompt = state.build_prompt();

        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();

                // Skip empty lines
                if line.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(line);

                // Parse and execute command
                match ShellCommand::parse_line(line) {
                    Ok(cmd) => {
                        if let Err(e) = state.execute(cmd).await {
                            eprintln!("{} {}", "Error:".red().bold(), e);
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Parse error:".red().bold(), e);
                        eprintln!("Type 'help' for available commands");
                    }
                }

                // Check if we should exit
                if state.should_exit {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    if let Some(path) = history_path {
        let _ = rl.save_history(&path);
    }

    println!("\n{}", "Goodbye!".bright_cyan());
    Ok(())
}

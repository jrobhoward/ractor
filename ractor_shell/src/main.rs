use anyhow::Result;
use clap::{Parser, ValueEnum};
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::IsTerminal;
use std::process::ExitCode;

use ractor_shell::config::ShellConfig;
use ractor_shell::{ShellCommand, ShellState};

/// Color output mode
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ColorMode {
    /// Automatically detect terminal (default)
    #[default]
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

/// Output format for scripting mode
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable tables (default)
    #[default]
    Table,
    /// JSON output for scripting
    Json,
}

#[derive(Parser)]
#[command(name = "ractor-shell")]
#[command(about = "Interactive shell for Ractor actor systems", long_about = None)]
struct Cli {
    /// Connect to a remote node on startup
    #[arg(short, long)]
    connect: Option<String>,

    /// Control colored output
    #[arg(long, value_enum, default_value = "auto")]
    color: ColorMode,

    /// Execute a single command and exit (non-interactive mode)
    #[arg(short = 'x', long)]
    execute: Option<String>,

    /// Output format (for scripting)
    #[arg(short, long, value_enum, default_value = "table")]
    format: OutputFormat,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    // Load configuration
    let config = ShellConfig::load();

    // Determine if we're in non-interactive (scripting) mode
    let non_interactive = cli.execute.is_some();
    let json_output = matches!(cli.format, OutputFormat::Json);

    // Configure colored output based on CLI flag, config, and terminal detection
    // CLI flag takes precedence over config
    // In non-interactive mode with JSON output, disable colors by default
    match cli.color {
        ColorMode::Always => colored::control::set_override(true),
        ColorMode::Never => colored::control::set_override(false),
        ColorMode::Auto => {
            if non_interactive || json_output {
                // Disable colors in scripting mode
                colored::control::set_override(false);
            } else if let Some(enabled) = config.get_color_enabled() {
                colored::control::set_override(enabled);
            } else {
                // Auto-detect: disable colors if stdout is not a terminal (e.g., piped)
                if !std::io::stdout().is_terminal() {
                    colored::control::set_override(false);
                }
            }
        }
    }

    // Initialize shell state (quiet mode for non-interactive)
    let mut state = if non_interactive {
        ShellState::new_quiet().await?
    } else {
        ShellState::new().await?
    };

    // Handle non-interactive mode: execute single command and exit
    if let Some(ref command) = cli.execute {
        // Auto-connect if specified (silently in non-interactive mode)
        let auto_connect = cli.connect.or(config.auto_connect.clone());
        if let Some(ref addr) = auto_connect {
            if let Err(e) = state
                .execute(ShellCommand::Connect { host: addr.clone() })
                .await
            {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({
                            "success": false,
                            "error": format!("Failed to connect to {}: {}", addr, e)
                        })
                    );
                } else {
                    eprintln!("Failed to connect to {}: {}", addr, e);
                }
                return Ok(ExitCode::FAILURE);
            }
        }

        // Parse and execute the command
        match ShellCommand::parse_line(command) {
            Ok(cmd) => {
                if let Err(e) = state.execute(cmd).await {
                    if json_output {
                        println!(
                            "{}",
                            serde_json::json!({
                                "success": false,
                                "error": e.to_string()
                            })
                        );
                    } else {
                        eprintln!("{} {}", "Error:".red().bold(), e);
                    }
                    return Ok(ExitCode::FAILURE);
                }
            }
            Err(e) => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({
                            "success": false,
                            "error": format!("Parse error: {}", e)
                        })
                    );
                } else {
                    eprintln!("{} {}", "Parse error:".red().bold(), e);
                }
                return Ok(ExitCode::FAILURE);
            }
        }

        return Ok(ExitCode::SUCCESS);
    }

    // Interactive mode - show banner
    println!("{}", "Ractor Shell v0.1.0".bright_cyan().bold());
    println!("Type 'help' for available commands, 'exit' to quit\n");

    // Auto-connect: CLI flag takes precedence over config
    let auto_connect = cli.connect.or(config.auto_connect.clone());
    if let Some(ref addr) = auto_connect {
        if let Err(e) = state
            .execute(ShellCommand::Connect { host: addr.clone() })
            .await
        {
            eprintln!(
                "{} Failed to connect to {}: {}",
                "Error:".red().bold(),
                addr,
                e
            );
        }
    }

    // Set up readline editor with history
    let mut rl = DefaultEditor::new()?;
    let history_path = config.history_path();

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
    Ok(ExitCode::SUCCESS)
}

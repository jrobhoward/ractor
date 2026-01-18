//! Unified Cluster Demo for ractor_shell
//!
//! This example demonstrates a complete Raft cluster with leader election,
//! providing both cluster nodes and an interactive shell for introspection.
//!
//! ## Quick Start (3-Node Cluster)
//!
//! Use the test script to start a 3-node cluster:
//! ```bash
//! ./ractor_shell/scripts/test_cluster.sh
//! ```
//!
//! Or manually:
//! ```bash
//! # Terminal 1: Start first node (seed node)
//! cargo run --example cluster_demo -p ractor_shell -- node --port 9001 --name node_a
//!
//! # Terminal 2: Start second node (connects to first)
//! cargo run --example cluster_demo -p ractor_shell -- node --port 9002 --name node_b --peer 127.0.0.1:9001
//!
//! # Terminal 3: Start third node
//! cargo run --example cluster_demo -p ractor_shell -- node --port 9003 --name node_c --peer 127.0.0.1:9001
//!
//! # Terminal 4: Start the shell
//! cargo run --example cluster_demo -p ractor_shell -- shell
//! ```
//!
//! ## Shell Commands
//!
//! Once the shell is running, connect to a node and query Raft status:
//! ```text
//! ractor@local > connect 127.0.0.1:9001
//! ractor@127.0.0.1:9001 > call raft_node IsLeader {}
//! ractor@127.0.0.1:9001 > call raft_node GetLeader {}
//! ractor@127.0.0.1:9001 > call raft_node GetStatus {}
//! ractor@127.0.0.1:9001 > call raft_node GetPeers {}
//! ```
//!
//! ## Modes
//!
//! - `node` - Run as a cluster node with Raft leader election
//! - `shell` - Run the interactive shell (pure client, must connect to nodes)

#[path = "cluster_demo/mod.rs"]
mod cluster_demo;

use std::time::Duration;

use clap::{Parser, Subcommand};
use colored::Colorize;
use rustyline::{Config, Editor};
use tokio::signal;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use ractor::Actor;
use ractor_cluster::node::{client, NodeConnectionMode};
use ractor_cluster::NodeServer;

use cluster_demo::raft::{RaftConfig, RaftNode, RAFT_CLUSTER_GROUP};
use cluster_demo::{DemoActor, DynamicDemoActor};
use ractor_shell::completer::{get_known_process_groups, update_completer_state, ShellHelper};
use ractor_shell::config::ShellConfig;
use ractor_shell::introspection::{IntrospectionActor, IntrospectionArgs};
use ractor_shell::{ShellCommand, ShellState};

/// Unified cluster demo for ractor_shell
#[derive(Parser, Debug)]
#[command(name = "cluster_demo")]
#[command(about = "Ractor shell cluster demo with Raft leader election")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run as a cluster node with Raft leader election
    Node {
        /// Port to listen on
        #[arg(short, long, default_value = "9001")]
        port: u16,

        /// Node name for identification
        #[arg(short, long, default_value = "node_a")]
        name: String,

        /// Cluster authentication cookie
        #[arg(short, long, default_value = "secret_cookie")]
        cookie: String,

        /// Peer node address to connect to (e.g., 127.0.0.1:9001)
        #[arg(long)]
        peer: Option<String>,

        /// Additional peer addresses (can specify multiple)
        #[arg(long = "peers")]
        additional_peers: Vec<String>,

        /// Election timeout minimum in ms
        #[arg(long, default_value = "150")]
        election_timeout_min: u64,

        /// Election timeout maximum in ms
        #[arg(long, default_value = "300")]
        election_timeout_max: u64,

        /// Heartbeat interval in ms
        #[arg(long, default_value = "100")]
        heartbeat_interval: u64,
    },

    /// Run the interactive shell (pure client mode)
    Shell {
        /// Immediately connect to a node (e.g., 127.0.0.1:9001)
        #[arg(short, long)]
        connect: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Node {
            port,
            name,
            cookie,
            peer,
            additional_peers,
            election_timeout_min,
            election_timeout_max,
            heartbeat_interval,
        } => {
            run_node(
                port,
                name,
                cookie,
                peer,
                additional_peers,
                election_timeout_min,
                election_timeout_max,
                heartbeat_interval,
            )
            .await
        }
        Commands::Shell { connect } => run_shell(connect).await,
    }
}

/// Run as a cluster node with Raft leader election
async fn run_node(
    port: u16,
    name: String,
    cookie: String,
    peer: Option<String>,
    additional_peers: Vec<String>,
    election_timeout_min: u64,
    election_timeout_max: u64,
    heartbeat_interval: u64,
) -> anyhow::Result<()> {
    // Initialize tracing with ShellTracingLayer for remote tracing support
    let (tracing_layer, tracing_handle) = ractor_shell::tracing::ShellTracingLayer::new();

    // Apply filter only to fmt layer so ShellTracingLayer sees ALL events
    let fmt_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("ractor_shell=info".parse()?);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
        .with(tracing_layer)
        .init();

    println!("{}", "=".repeat(60).bright_black());
    println!(
        "{}",
        format!("  Raft Cluster Node: {}", name)
            .bright_cyan()
            .bold()
    );
    println!("{}", "=".repeat(60).bright_black());
    println!();

    // Get hostname for identification
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string());

    // Start the NodeServer with transitive connection mode
    println!("Starting NodeServer on port {} (transitive mode)...", port);

    let node_server_actor = NodeServer::new(
        port,
        cookie.clone(),
        name.clone(),
        hostname,
        None,
        Some(NodeConnectionMode::Transitive),
    );

    let (node_server, _handle) =
        Actor::spawn(Some("node_server".to_string()), node_server_actor, ()).await?;

    println!("{} NodeServer listening on 0.0.0.0:{}", "OK".green(), port);

    // Start the IntrospectionActor so shells can connect
    println!("Starting IntrospectionActor...");

    let (_introspection_ref, _) = Actor::spawn(
        Some("introspection".to_string()),
        IntrospectionActor,
        IntrospectionArgs {
            node_name: name.clone(),
            tracing_handle: Some(tracing_handle),
        },
    )
    .await?;

    println!(
        "{} IntrospectionActor ready (remote tracing enabled)",
        "OK".green()
    );

    // Start demo actors for testing
    println!("Starting demo actors...");

    let (demo_ref, _): (ractor::ActorRef<cluster_demo::DemoMessage>, _) =
        Actor::spawn(Some("demo_actor".to_string()), DemoActor, ()).await?;
    let (dynamic_ref, _): (ractor::ActorRef<ractor_shell::dynamic::DynamicMessage>, _) =
        Actor::spawn(Some("dynamic_actor".to_string()), DynamicDemoActor, ()).await?;

    // Join demo actors to a process group
    ractor::pg::join(
        "demo_group".to_string(),
        vec![demo_ref.get_cell(), dynamic_ref.get_cell()],
    );

    println!(
        "{} Demo actors ready (demo_actor, dynamic_actor)",
        "OK".green()
    );

    // Start the Raft node for leader election
    println!("Starting RaftNode...");

    let raft_config = RaftConfig {
        node_name: name.clone(),
        election_timeout_min_ms: election_timeout_min,
        election_timeout_max_ms: election_timeout_max,
        heartbeat_interval_ms: heartbeat_interval,
    };

    let (_raft_ref, _) = Actor::spawn(Some("raft_node".to_string()), RaftNode, raft_config).await?;

    println!(
        "{} RaftNode ready (joined '{}' group)",
        "OK".green(),
        RAFT_CLUSTER_GROUP
    );
    println!();

    // Connect to peer nodes if specified
    let mut peers_to_connect = Vec::new();
    if let Some(p) = &peer {
        peers_to_connect.push(p.clone());
    }
    peers_to_connect.extend(additional_peers.clone());

    if !peers_to_connect.is_empty() {
        println!("Connecting to peer nodes...");
        tokio::time::sleep(Duration::from_millis(100)).await;

        for peer_addr in &peers_to_connect {
            println!("  Connecting to {}...", peer_addr.cyan());
            match client::connect(&node_server, peer_addr.as_str()).await {
                Ok(()) => println!("    {} Connected to {}", "OK".green(), peer_addr),
                Err(e) => println!(
                    "    {} Failed to connect to {}: {:?}",
                    "FAIL".red(),
                    peer_addr,
                    e
                ),
            }
        }
        println!();
    }

    // Print status and instructions
    print_node_instructions(&name, port, &cookie);

    // Wait for shutdown signal
    signal::ctrl_c().await?;

    println!();
    println!("Shutting down...");

    node_server.stop(Some("Shutdown".to_string()));
    tokio::time::sleep(Duration::from_millis(500)).await;

    println!("{} Node stopped.", "OK".green());
    Ok(())
}

fn print_node_instructions(name: &str, port: u16, cookie: &str) {
    println!("{}", "-".repeat(60).bright_black());
    println!("{}", "  Raft Cluster Status".bold());
    println!("{}", "-".repeat(60).bright_black());
    println!();
    println!("  Node: {}", name.cyan());
    println!("  Port: {}", port.to_string().cyan());
    println!(
        "  Cookie: {}",
        cookie.chars().take(4).collect::<String>() + "..."
    );
    println!();

    println!("{}", "-".repeat(60).bright_black());
    println!("{}", "  Shell Commands".bold());
    println!("{}", "-".repeat(60).bright_black());
    println!();
    println!("  From another terminal:");
    println!();
    println!(
        "    {}",
        "cargo run --example cluster_demo -p ractor_shell -- shell".bright_white()
    );
    println!();
    println!("  Then query this node:");
    println!();
    println!("    {}", format!("connect 127.0.0.1:{}", port).green());
    println!("    {}", "call raft_node IsLeader {}".green());
    println!("    {}", "call raft_node GetLeader {}".green());
    println!("    {}", "call raft_node GetStatus {}".green());
    println!("    {}", "call raft_node GetPeers {}".green());
    println!();

    println!("{}", "-".repeat(60).bright_black());
    println!("{}", "  Demo Actors".bold());
    println!("{}", "-".repeat(60).bright_black());
    println!();
    println!("  {}", "registry".green());
    println!("  {}", "pg members demo_group".green());
    println!(
        "  {}",
        r#"call dynamic_actor {"command": "get_status"}"#.green()
    );
    println!(
        "  {}",
        r#"send dynamic_actor {"command": "increment"}"#.green()
    );
    println!();

    println!("{}", "-".repeat(60).bright_black());
    println!();
    println!(
        "{} Node ready. Press {} to shut down.",
        "OK".green().bold(),
        "Ctrl+C".yellow()
    );
    println!();
}

/// Run the interactive shell (pure client mode)
async fn run_shell(connect: Option<String>) -> anyhow::Result<()> {
    println!("{}", "Ractor Shell - Cluster Demo".bright_cyan().bold());
    println!();
    println!("This shell is a pure client. Connect to a cluster node to begin:");
    println!();
    println!("  {}  - Connect to node", "connect 127.0.0.1:9001".green());
    println!("  {}               - List connected nodes", "nodes".green());
    println!("  {}                - Show help", "help".green());
    println!();

    // Initialize shell
    let mut state = ShellState::new().await?;

    // If a connect address was provided, connect immediately
    if let Some(addr) = connect {
        println!("Connecting to {}...", addr.cyan());
        if let Err(e) = state
            .execute(ShellCommand::Connect { host: addr.clone() })
            .await
        {
            eprintln!("{} Failed to connect: {}", "Error:".red().bold(), e);
        }
    }

    // Load config and create editor
    let config = ShellConfig::load();
    let rl_config = Config::builder()
        .edit_mode(config.get_edit_mode())
        .completion_type(config.get_completion_type())
        .build();

    let helper = ShellHelper::new();
    let mut rl = Editor::with_config(rl_config)?;
    rl.set_helper(Some(helper));

    let history_path = config.history_path();

    // Load history if it exists
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

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

    // Save history
    if let Some(path) = history_path {
        let _ = rl.save_history(&path);
    }

    println!("\n{}", "Goodbye!".bright_cyan());
    Ok(())
}

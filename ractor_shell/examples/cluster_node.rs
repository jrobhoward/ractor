//! Raft Cluster Node for ractor_shell
//!
//! This example demonstrates a cluster node with Raft-based leader election.
//! Nodes automatically elect a leader and can report their status via shell commands.
//!
//! ## Quick Start (3-Node Cluster)
//!
//! Use the test script to start a 3-node cluster:
//! ```bash
//! ./ractor_shell/scripts/test_cluster.sh
//! ```
//!
//! Or manually start nodes:
//! ```bash
//! # Terminal 1: Node A (leader candidate)
//! cargo run --example cluster_node -p ractor_shell -- --name node_a --port 9001
//!
//! # Terminal 2: Node B (connects to A)
//! cargo run --example cluster_node -p ractor_shell -- --name node_b --port 9002 --peer 127.0.0.1:9001
//!
//! # Terminal 3: Node C (connects to A, discovers B)
//! cargo run --example cluster_node -p ractor_shell -- --name node_c --port 9003 --peer 127.0.0.1:9001
//! ```
//!
//! ## Shell Commands
//!
//! Query any node's Raft status:
//! ```bash
//! cargo run --example demo -p ractor_shell
//!
//! # Connect and query using typed RPC
//! ractor@local > connect 127.0.0.1:9001
//! ractor@127.0.0.1:9001 > call raft_node IsLeader {}
//! ractor@127.0.0.1:9001 > call raft_node GetLeader {}
//! ractor@127.0.0.1:9001 > call raft_node GetStatus {}
//! ractor@127.0.0.1:9001 > call raft_node GetPeers {}
//! ```
//!
//! ## Architecture
//!
//! Each node runs:
//! - `NodeServer`: Handles cluster networking
//! - `IntrospectionActor`: Enables shell connectivity
//! - `RaftNode`: Participates in leader election
//!
//! Nodes use transitive connection mode, so connecting to one node
//! automatically discovers and connects to its peers.

use std::time::Duration;

use clap::Parser;
use colored::Colorize;
use tokio::signal;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use ractor::Actor;
use ractor_cluster::node::{client, NodeConnectionMode};
use ractor_cluster::NodeServer;

use ractor_shell::introspection::{IntrospectionActor, IntrospectionArgs};
use ractor_shell::raft::{RaftConfig, RaftNode};

/// Command line arguments for the cluster node
#[derive(Parser, Debug)]
#[command(name = "cluster_node")]
#[command(about = "A ractor_cluster node with Raft leader election")]
struct Args {
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

    /// Heartbeat interval in ms (lower = faster leader detection, but more network traffic)
    #[arg(long, default_value = "100")]
    heartbeat_interval: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize tracing with ShellTracingLayer for remote tracing support
    let (tracing_layer, tracing_handle) = ractor_shell::tracing::ShellTracingLayer::new();

    // Apply filter only to fmt layer so ShellTracingLayer sees ALL events
    // ShellTracingLayer does its own pattern-based filtering
    let fmt_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("ractor_shell=info".parse()?); // fmt only shows INFO+

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
        .with(tracing_layer) // No filter - sees everything
        .init();

    println!("{}", "═".repeat(60).bright_black());
    println!(
        "{}",
        format!("  Raft Cluster Node: {}", args.name)
            .bright_cyan()
            .bold()
    );
    println!("{}", "═".repeat(60).bright_black());
    println!();

    // Get hostname for identification
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string());

    // Start the NodeServer with transitive connection mode
    // This allows nodes to discover each other automatically
    println!(
        "Starting NodeServer on port {} (transitive mode)...",
        args.port
    );

    let node_server_actor = NodeServer::new(
        args.port,
        args.cookie.clone(),
        args.name.clone(),
        hostname,
        None, // encryption_mode
        Some(NodeConnectionMode::Transitive),
    );

    let (node_server, _handle) =
        Actor::spawn(Some("node_server".to_string()), node_server_actor, ()).await?;

    println!(
        "{} NodeServer listening on 0.0.0.0:{}",
        "✓".green(),
        args.port
    );

    // Start the IntrospectionActor so shells can connect
    println!("Starting IntrospectionActor...");

    let (_introspection_ref, _) = Actor::spawn(
        Some("introspection".to_string()),
        IntrospectionActor,
        IntrospectionArgs {
            node_name: args.name.clone(),
            tracing_handle: Some(tracing_handle),
        },
    )
    .await?;

    println!(
        "{} IntrospectionActor ready (remote tracing enabled)",
        "✓".green()
    );

    // Start the Raft node for leader election
    println!("Starting RaftNode...");

    let raft_config = RaftConfig {
        node_name: args.name.clone(),
        election_timeout_min_ms: args.election_timeout_min,
        election_timeout_max_ms: args.election_timeout_max,
        heartbeat_interval_ms: args.heartbeat_interval,
    };

    let (_raft_ref, _) = Actor::spawn(Some("raft_node".to_string()), RaftNode, raft_config).await?;

    println!(
        "{} RaftNode ready (joined 'raft_cluster' group)",
        "✓".green()
    );
    println!();

    // Connect to peer nodes if specified
    let mut peers_to_connect = Vec::new();
    if let Some(peer) = &args.peer {
        peers_to_connect.push(peer.clone());
    }
    peers_to_connect.extend(args.additional_peers.clone());

    if !peers_to_connect.is_empty() {
        println!("Connecting to peer nodes...");
        // Give our server a moment to fully start
        tokio::time::sleep(Duration::from_millis(100)).await;

        for peer_addr in &peers_to_connect {
            println!("  Connecting to {}...", peer_addr.cyan());
            match client::connect(&node_server, peer_addr.as_str()).await {
                Ok(()) => println!("    {} Connected to {}", "✓".green(), peer_addr),
                Err(e) => println!(
                    "    {} Failed to connect to {}: {:?}",
                    "✗".red(),
                    peer_addr,
                    e
                ),
            }
        }
        println!();
    }

    // Print status and instructions
    print_instructions(&args);

    // Wait for shutdown signal
    signal::ctrl_c().await?;

    println!();
    println!("Shutting down...");

    // Stop actors gracefully - node_server will clean up children
    node_server.stop(Some("Shutdown".to_string()));

    // Give actors time to clean up
    tokio::time::sleep(Duration::from_millis(500)).await;

    println!("{} Node stopped.", "✓".green());
    Ok(())
}

fn print_instructions(args: &Args) {
    println!("{}", "─".repeat(60).bright_black());
    println!("{}", "  Raft Cluster Status".bold());
    println!("{}", "─".repeat(60).bright_black());
    println!();
    println!("  Node: {}", args.name.cyan());
    println!("  Port: {}", args.port.to_string().cyan());
    println!(
        "  Cookie: {}",
        args.cookie.chars().take(4).collect::<String>() + "..."
    );
    println!();

    println!("{}", "─".repeat(60).bright_black());
    println!("{}", "  Shell Commands".bold());
    println!("{}", "─".repeat(60).bright_black());
    println!();
    println!("  From another terminal:");
    println!();
    println!(
        "    {}",
        "cargo run --example demo -p ractor_shell".bright_white()
    );
    println!();
    println!("  Then query this node:");
    println!();
    println!("    {}", format!("connect 127.0.0.1:{}", args.port).green());
    println!("    {}", "call raft_node IsLeader {}".green());
    println!("    {}", "call raft_node GetLeader {}".green());
    println!("    {}", "call raft_node GetStatus {}".green());
    println!("    {}", "call raft_node GetPeers {}".green());
    println!();

    println!("{}", "─".repeat(60).bright_black());
    println!("{}", "  Raft Commands Reference".bold());
    println!("{}", "─".repeat(60).bright_black());
    println!();
    println!(
        "  {} - Check if this node is leader",
        "IsLeader {}".yellow()
    );
    println!("  {} - Get current leader name", "GetLeader {}".yellow());
    println!(
        "  {} - Full status (role, term, peers)",
        "GetStatus {}".yellow()
    );
    println!("  {} - List connected peers", "GetPeers {}".yellow());
    println!();

    println!("{}", "─".repeat(60).bright_black());
    println!();
    println!(
        "{} Node ready. Press {} to shut down.",
        "✓".green().bold(),
        "Ctrl+C".yellow()
    );
    println!();
}

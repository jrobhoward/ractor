//! Cluster Node Example for ractor_shell
//!
//! This example demonstrates setting up a remote node that can be connected to
//! via ractor_shell. Run this first, then connect to it from another shell instance.
//!
//! ## Usage
//!
//! Terminal 1 - Start the node server:
//! ```bash
//! cargo run --example cluster_node -- --port 9002 --name node_b
//! ```
//!
//! Terminal 2 - Connect from shell:
//! ```bash
//! cargo run --example demo -p ractor_shell
//! # Then in the shell:
//! ractor@local > connect 127.0.0.1:9002
//! ractor@local > use 127.0.0.1:9002
//! ractor@127.0.0.1:9002 > registry
//! ractor@127.0.0.1:9002 > cluster
//! ```
//!
//! ## What This Demonstrates
//!
//! - Setting up a ractor_cluster NodeServer
//! - Spawning an IntrospectionActor for shell connectivity
//! - Creating actors that participate in process groups
//! - Remote shell introspection

use clap::Parser;
use colored::Colorize;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use ractor_cluster::NodeServer;
use ractor_shell::dynamic::{CallResponse, DynamicMessage};
use ractor_shell::introspection::IntrospectionActor;
use serde_json::json;
use std::time::Duration;
use tokio::signal;

/// Command line arguments for the cluster node
#[derive(Parser, Debug)]
#[command(name = "cluster_node")]
#[command(about = "A ractor_cluster node that can be connected to via ractor_shell")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "9002")]
    port: u16,

    /// Node name for identification
    #[arg(short, long, default_value = "node_b")]
    name: String,

    /// Cluster authentication cookie
    #[arg(short, long, default_value = "secret_cookie")]
    cookie: String,
}

/// Example actor that runs on the remote node
/// Demonstrates an actor that can receive dynamic messages from the shell
struct RemoteWorker {
    name: String,
}

struct WorkerState {
    processed_count: u64,
    status: String,
}

impl Actor for RemoteWorker {
    type Msg = DynamicMessage;
    type State = WorkerState;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        // Join a process group so we're discoverable
        ractor::pg::join("workers".to_string(), vec![myself.get_cell()]);

        println!(
            "  [{}] Worker started and joined 'workers' group",
            self.name
        );
        Ok(WorkerState {
            processed_count: 0,
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
                state.processed_count += 1;
                println!(
                    "  [{}] Received cast #{}: {}",
                    self.name, state.processed_count, json
                );

                if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
                    state.status = status.to_string();
                    println!("  [{}] Status changed to: {}", self.name, status);
                }
            }
            DynamicMessage::Call(json, reply) => {
                state.processed_count += 1;
                println!(
                    "  [{}] Received call #{}: {}",
                    self.name, state.processed_count, json
                );

                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "status" => {
                            let response = json!({
                                "name": self.name,
                                "processed": state.processed_count,
                                "status": state.status
                            });
                            let _ = reply.send(CallResponse::Success(response));
                        }
                        "work" => {
                            // Simulate some work
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            let response = json!({
                                "result": "work completed",
                                "total_processed": state.processed_count
                            });
                            let _ = reply.send(CallResponse::Success(response));
                        }
                        _ => {
                            let _ = reply
                                .send(CallResponse::Error(format!("Unknown command: {}", cmd)));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error("Missing 'command' field".to_string()));
                }
            }
            DynamicMessage::Ping(reply) => {
                let _ = reply.send(true);
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("{}", "═".repeat(60).bright_black());
    println!(
        "{}",
        format!("  Ractor Cluster Node: {}", args.name)
            .bright_cyan()
            .bold()
    );
    println!("{}", "═".repeat(60).bright_black());
    println!();

    // Start the NodeServer
    println!("Starting NodeServer on port {}...", args.port);

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string());

    let node_server_actor = NodeServer::new(
        args.port,
        args.cookie.clone(),
        args.name.clone(),
        hostname,
        None, // encryption_mode
        None, // connection_mode
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

    let (introspection_ref, _) = Actor::spawn(
        Some("introspection".to_string()),
        IntrospectionActor,
        args.name.clone(),
    )
    .await?;

    println!(
        "{} IntrospectionActor ready (joined 'ractor_shell_introspection' group)",
        "✓".green()
    );

    // Spawn some worker actors
    println!("Spawning worker actors...");

    for i in 1..=3 {
        let name = format!("worker_{}", i);
        let worker = RemoteWorker { name: name.clone() };
        let (_ref, _handle) = Actor::spawn(Some(name.clone()), worker, ()).await?;
    }

    println!("{} 3 worker actors spawned", "✓".green());
    println!();

    // Print connection instructions
    println!("{}", "─".repeat(60).bright_black());
    println!("{}", "  Connection Instructions".bold());
    println!("{}", "─".repeat(60).bright_black());
    println!();
    println!("  From another terminal, run:");
    println!();
    println!(
        "    {}",
        "cargo run --example demo -p ractor_shell".bright_white()
    );
    println!();
    println!("  Then in the shell:");
    println!();
    println!("    {}", format!("connect 127.0.0.1:{}", args.port).green());
    println!("    {}", format!("use 127.0.0.1:{}", args.port).green());
    println!("    {}", "registry".green());
    println!("    {}", "actors".green());
    println!("    {}", "pg members workers".green());
    println!("    {}", r#"call worker_1 {"command": "status"}"#.green());
    println!("    {}", "cluster".green());
    println!();
    println!("{}", "─".repeat(60).bright_black());
    println!();
    println!(
        "{} Node ready. Press {} to shut down.",
        "✓".green().bold(),
        "Ctrl+C".yellow()
    );
    println!();

    // Wait for shutdown signal
    signal::ctrl_c().await?;

    println!();
    println!("Shutting down...");

    // Stop actors gracefully
    introspection_ref.stop(Some("Shutdown".to_string()));
    node_server.stop(Some("Shutdown".to_string()));

    // Give actors time to clean up
    tokio::time::sleep(Duration::from_millis(500)).await;

    println!("{} Node stopped.", "✓".green());
    Ok(())
}

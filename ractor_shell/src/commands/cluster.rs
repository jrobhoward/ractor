//! Cluster Commands
//!
//! Commands for connecting to and managing remote nodes:
//! - `connect` - Connect to a remote node
//! - `disconnect` - Disconnect from a node
//! - `reconnect` - Reconnect to a node
//! - `nodes` - List connected nodes
//! - `use` - Switch to a node context
//! - `cluster` - Show cluster topology
//! - `ping` - Ping a remote node

use std::collections::HashMap;

use colored::Colorize;
use ractor::rpc::CallResult;
use ractor::{pg, Actor, ActorRef};
use tabled::Tabled;

use crate::error::{ShellError, ShellResult};
use crate::introspection::INTROSPECTION_GROUP;
use crate::protocol::ShellProtocolMessage;
use crate::ShellState;
use crate::{table, DEFAULT_RPC_TIMEOUT};

impl ShellState {
    /// Connect to a remote node.
    pub(crate) async fn cmd_connect(&mut self, host: String) -> ShellResult<()> {
        println!("{} {}", "Connecting to".bright_black(), host.green());

        // Spawn NodeServer if not already running
        // Default port: 9100, default cookie: "secret_cookie"
        const LOCAL_NODE_SERVER_PORT: u16 = 9100;
        const LOCAL_CLUSTER_COOKIE: &str = "secret_cookie";

        if self.node_server.is_none() {
            println!("  Starting local NodeServer...");

            let node_server = ractor_cluster::NodeServer::new(
                LOCAL_NODE_SERVER_PORT,
                LOCAL_CLUSTER_COOKIE.to_string(),
                format!("{}_instance", self.local_node_name),
                self.local_node_name.clone(),
                None, // no encryption
                None, // default connection mode
            );

            let (node_ref, _node_handle) = Actor::spawn(None, node_server, ()).await?;
            self.node_server = Some(node_ref.clone());

            println!(
                "  {} NodeServer started on port {}",
                "✓".green(),
                LOCAL_NODE_SERVER_PORT
            );
        }

        // Connect to the remote node
        if let Some(ref node_ref) = self.node_server {
            ractor_cluster::client_connect(node_ref, &host)
                .await
                .map_err(|e| ShellError::ClusterError(format!("{:?}", e)))?;
            println!("  {} Connected to {}", "✓".green(), host);

            // Wait a bit for connection to establish
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Discover introspection actor in the well-known group
            println!("  Discovering introspection actor...");
            let members = pg::get_members(&INTROSPECTION_GROUP.to_string());

            // Get IDs of introspection actors we've already stored
            let known_actor_ids: std::collections::HashSet<_> = self
                .connected_nodes
                .values()
                .map(|actor_ref| actor_ref.get_id())
                .collect();

            // Find remote introspection actors we haven't seen yet
            let new_remote_introspection: Vec<ActorRef<ShellProtocolMessage>> = members
                .into_iter()
                .filter(|cell| !cell.get_id().is_local())
                .filter(|cell| !known_actor_ids.contains(&cell.get_id()))
                .map(ActorRef::<ShellProtocolMessage>::from)
                .collect();

            // If we found new actors, ping each to identify them
            // If no new actors, we're connecting to a node in an already-connected cluster
            let introspection_ref = if !new_remote_introspection.is_empty() {
                // Ping each new actor to find one that responds
                let mut found_ref = None;
                for candidate in &new_remote_introspection {
                    if let Ok(CallResult::Success(_pong)) = candidate
                        .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
                        .await
                    {
                        found_ref = Some(candidate.clone());
                        break;
                    }
                }
                found_ref.ok_or(ShellError::NoIntrospectionActor)?
            } else {
                // No new actors - this node is part of an already-connected cluster
                // All introspection actors are already known, find one we haven't stored yet
                let all_remote: Vec<ActorRef<ShellProtocolMessage>> =
                    pg::get_members(&INTROSPECTION_GROUP.to_string())
                        .into_iter()
                        .filter(|cell| !cell.get_id().is_local())
                        .filter(|cell| !known_actor_ids.contains(&cell.get_id()))
                        .map(ActorRef::<ShellProtocolMessage>::from)
                        .collect();

                // Find an actor we haven't stored yet
                let mut found_ref = None;
                for candidate in &all_remote {
                    if let Ok(CallResult::Success(_pong)) = candidate
                        .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
                        .await
                    {
                        found_ref = Some(candidate.clone());
                        break;
                    }
                }

                // If we still didn't find it, all nodes may already be connected
                found_ref.ok_or_else(|| {
                    ShellError::ClusterError(format!(
                        "Could not identify introspection actor for {}. \
                         This node may already be connected via another address.",
                        host
                    ))
                })?
            };

            // Test with a ping and display result
            let pong_result = introspection_ref
                .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
                .await
                .map_err(ShellError::messaging)?;

            match pong_result {
                CallResult::Success(pong) => {
                    println!("  {} {}", "✓".green(), pong.bright_black());
                }
                CallResult::Timeout => {
                    return Err(ShellError::rpc_timeout());
                }
                CallResult::SenderError => {
                    return Err(ShellError::RpcSenderError);
                }
            }

            // Store the connection (using host as node name for now)
            self.connected_nodes
                .insert(host.clone(), introspection_ref.clone());

            println!(
                "{} Connected to node at {}",
                "✓".green().bold(),
                host.green()
            );

            // Automatically discover cluster topology
            println!("  Discovering cluster topology...");
            let topo_result = introspection_ref
                .call(
                    ShellProtocolMessage::GetClusterTopology,
                    Some(DEFAULT_RPC_TIMEOUT),
                )
                .await
                .map_err(ShellError::messaging)?;

            match topo_result {
                CallResult::Success(topology) => {
                    let node_count = topology.nodes.len();
                    let group_count = topology.process_groups.len();
                    self.cluster_topology = Some(topology);
                    println!(
                        "  {} Discovered {} nodes and {} process groups",
                        "✓".green(),
                        node_count,
                        group_count
                    );
                }
                CallResult::Timeout => {
                    println!("  {} Topology discovery timeout (non-fatal)", "⚠".yellow());
                }
                CallResult::SenderError => {
                    println!("  {} Topology discovery failed (non-fatal)", "⚠".yellow());
                }
            }

            // Auto-switch to the connected node
            self.current_node = Some(host.clone());
            println!("{} Now using {}", "✓".green().bold(), host.yellow());
        }

        Ok(())
    }

    /// Disconnect from a remote node.
    pub(crate) async fn cmd_disconnect(&mut self, node: String) -> ShellResult<()> {
        if self.connected_nodes.remove(&node).is_some() {
            // If we were using this node, switch back to local
            if self.current_node.as_ref() == Some(&node) {
                self.current_node = None;
                println!("{} Switched back to local context", "→".bright_black());
            }
            println!("{} Disconnected from {}", "✓".green(), node);
            Ok(())
        } else {
            Err(ShellError::NodeNotConnected(node))
        }
    }

    /// Reconnect to a remote node (disconnect + connect).
    pub(crate) async fn cmd_reconnect(&mut self, node: String) -> ShellResult<()> {
        // Remove the stale connection if it exists
        let was_current = self.current_node.as_ref() == Some(&node);
        if self.connected_nodes.remove(&node).is_some() {
            println!(
                "{} Disconnected stale connection to {}",
                "→".bright_black(),
                node
            );
        }

        // Reconnect
        self.cmd_connect(node.clone()).await?;

        // Restore as current node if it was before
        if was_current {
            self.current_node = Some(node.clone());
            println!("{} Restored {} as current node", "✓".green(), node);
        }

        Ok(())
    }

    /// List connected nodes.
    pub(crate) async fn cmd_nodes(&self) -> ShellResult<()> {
        if self.connected_nodes.is_empty() {
            println!("No connected nodes");
            println!(
                "\nUse {} to connect to a remote node",
                "connect <host:port>".green()
            );
            return Ok(());
        }

        println!(
            "{}",
            format!("Connected Nodes ({}):", self.connected_nodes.len()).bold()
        );
        for node_name in self.connected_nodes.keys() {
            let indicator = if self.current_node.as_ref() == Some(node_name) {
                "*".green()
            } else {
                "•".bright_cyan()
            };
            println!("  {} {}", indicator, node_name.green());
        }

        if self.current_node.is_none() {
            println!("\nCurrently using: {}", "local".yellow());
        }

        Ok(())
    }

    /// Switch context to a node.
    pub(crate) async fn cmd_use(&mut self, node: String) -> ShellResult<()> {
        if node == "local" {
            self.current_node = None;
            println!("{} Switched to local node", "✓".green());
            return Ok(());
        }

        if self.connected_nodes.contains_key(&node) {
            self.current_node = Some(node.clone());
            println!("{} Switched to node {}", "✓".green(), node.green());
            Ok(())
        } else {
            Err(ShellError::NodeNotConnected(node))
        }
    }

    /// Show cluster topology.
    pub(crate) async fn cmd_cluster(&mut self, subcommand: Option<String>) -> ShellResult<()> {
        // We need to be connected to a node to query cluster topology
        let node_name = match &self.current_node {
            Some(name) => name.clone(),
            None => {
                println!(
                    "{}",
                    "Not connected to any nodes. Use 'connect <host:port>' first.".yellow()
                );
                return Ok(());
            }
        };

        let introspection_ref = match self.connected_nodes.get(&node_name) {
            Some(r) => r.clone(),
            None => return Err(ShellError::NodeNotConnected(node_name)),
        };

        println!("{}", "Fetching cluster topology...".bright_black());
        let result = introspection_ref
            .call(
                ShellProtocolMessage::GetClusterTopology,
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        let topology = match result {
            CallResult::Success(topo) => {
                self.cluster_topology = Some(topo.clone());
                topo
            }
            CallResult::Timeout => return Err(ShellError::rpc_timeout()),
            CallResult::SenderError => return Err(ShellError::RpcSenderError),
        };

        // Handle subcommands
        match subcommand.as_deref() {
            None | Some("nodes") => {
                // Show cluster nodes
                println!();
                println!(
                    "{}",
                    format!("Cluster Nodes ({}):", topology.nodes.len()).bold()
                );
                println!();

                #[derive(Tabled)]
                struct NodeRow {
                    #[tabled(rename = "Node ID")]
                    id: String,
                    #[tabled(rename = "Node Name")]
                    name: String,
                    #[tabled(rename = "Actors")]
                    actors: String,
                    #[tabled(rename = "Local")]
                    is_local: String,
                }

                let rows: Vec<NodeRow> = topology
                    .nodes
                    .iter()
                    .map(|node| NodeRow {
                        id: node.id.clone(),
                        name: node.name.clone(),
                        actors: if node.is_local {
                            node.actor_count.to_string()
                        } else {
                            "-".to_string()
                        },
                        is_local: if node.is_local { "yes" } else { "no" }.to_string(),
                    })
                    .collect();

                let table = table::build_table(rows);
                println!("{}", table);
            }

            Some("groups") | Some("pg") => {
                // Show process groups across cluster
                println!();
                println!(
                    "{}",
                    format!("Process Groups ({}):", topology.process_groups.len()).bold()
                );
                println!();

                for (group_name, members) in &topology.process_groups {
                    println!("{}", format!("Group: {}", group_name).green().bold());

                    #[derive(Tabled)]
                    struct MemberRow {
                        #[tabled(rename = "Actor ID")]
                        id: String,
                        #[tabled(rename = "Name")]
                        name: String,
                        #[tabled(rename = "Node")]
                        node: String,
                    }

                    let rows: Vec<MemberRow> = members
                        .iter()
                        .map(|loc| MemberRow {
                            id: loc.actor_id.clone(),
                            name: loc.actor_name.clone().unwrap_or_else(|| "-".to_string()),
                            node: format!("{} ({})", loc.node_name, loc.node_id),
                        })
                        .collect();

                    let table = table::build_table(rows);
                    println!("{}", table);
                    println!();
                }

                if topology.process_groups.is_empty() {
                    println!("No process groups found in the cluster.");
                }
            }

            Some("actors") => {
                // Show all actors across cluster (from process groups)
                println!();
                println!("{}", "Actors Across Cluster:".bold());
                println!();

                let mut all_actors: Vec<_> = topology
                    .process_groups
                    .values()
                    .flat_map(|members| members.iter())
                    .collect();

                // Deduplicate by actor_id
                all_actors.sort_by(|a, b| a.actor_id.cmp(&b.actor_id));
                all_actors.dedup_by(|a, b| a.actor_id == b.actor_id);

                #[derive(Tabled)]
                struct ActorRow {
                    #[tabled(rename = "Actor ID")]
                    id: String,
                    #[tabled(rename = "Name")]
                    name: String,
                    #[tabled(rename = "Node")]
                    node: String,
                }

                let rows: Vec<ActorRow> = all_actors
                    .iter()
                    .map(|loc| ActorRow {
                        id: loc.actor_id.clone(),
                        name: loc.actor_name.clone().unwrap_or_else(|| "-".to_string()),
                        node: format!("{} ({})", loc.node_name, loc.node_id),
                    })
                    .collect();

                let table = table::build_table(rows);
                println!("{}", table);
                println!();
                println!(
                    "{}",
                    format!("Total: {} actors", all_actors.len()).bright_black()
                );
            }

            Some(unknown) => {
                println!(
                    "{} Unknown cluster subcommand: {}",
                    "Error:".red().bold(),
                    unknown
                );
                println!();
                println!("Available subcommands:");
                println!(
                    "  {} or {}  Show cluster nodes (default)",
                    "cluster".green(),
                    "cluster nodes".green()
                );
                println!("  {}       Show process groups", "cluster groups".green());
                println!("  {}       Show all actors", "cluster actors".green());
            }
        }

        Ok(())
    }

    /// Ping a remote node to measure latency.
    pub(crate) async fn cmd_ping(&mut self, node: String) -> ShellResult<()> {
        use std::time::Instant;

        // First, ensure we're connected to the node
        let introspection_ref = if let Some(existing_ref) = self.connected_nodes.get(&node) {
            existing_ref.clone()
        } else {
            // Auto-connect if not already connected
            println!("{} Connecting to {}...", "→".bright_black(), node.cyan());
            self.cmd_connect(node.clone()).await?;

            // Get the reference after connection
            self.connected_nodes
                .get(&node)
                .ok_or_else(|| ShellError::NodeNotConnected(node.clone()))?
                .clone()
        };

        // Measure round-trip time
        let start = Instant::now();
        let result = introspection_ref
            .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
            .await
            .map_err(ShellError::messaging)?;

        let elapsed = start.elapsed();

        match result {
            CallResult::Success(response) => {
                let latency_ms = elapsed.as_secs_f64() * 1000.0;
                println!("{} {} ({:.2}ms)", "✓".green(), response.green(), latency_ms);
            }
            CallResult::Timeout => {
                println!("{} Ping timeout after {:?}", "✗".red(), DEFAULT_RPC_TIMEOUT);
            }
            CallResult::SenderError => {
                println!("{} Ping failed: node unreachable", "✗".red());
            }
        }

        Ok(())
    }

    /// Show system statistics.
    pub(crate) async fn cmd_stats(&self) -> ShellResult<()> {
        use ractor::registry;

        println!();
        println!("{}", "System Statistics".bold().underline());
        println!();

        // Check if we're working with a remote node or local
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                println!("{} {}", "Node:".bright_black(), node_name.green());
                println!();

                // Get actor count via registry
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListRegisteredActors,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        println!("  {} {}", "Registered Actors:".bold(), actors.len());

                        // Count by status
                        let statuses: HashMap<String, usize> =
                            actors.iter().fold(HashMap::new(), |mut acc, actor| {
                                *acc.entry(actor.status.clone()).or_insert(0) += 1;
                                acc
                            });

                        for (status, count) in statuses {
                            println!("    {} {}", status.bright_black(), count);
                        }
                    }
                    _ => {
                        return Err(ShellError::RemoteOperationFailed(
                            "Failed to fetch stats".to_string(),
                        ))
                    }
                }

                println!();

                // Get cluster topology for more stats
                if let Some(ref topology) = self.cluster_topology {
                    println!("  {} {}", "Cluster Nodes:".bold(), topology.nodes.len());
                    println!(
                        "  {} {}",
                        "Process Groups:".bold(),
                        topology.process_groups.len()
                    );

                    let total_actors_in_groups: usize = topology
                        .process_groups
                        .values()
                        .map(|members| members.len())
                        .sum();
                    println!(
                        "  {} {}",
                        "Actors in Groups:".bold(),
                        total_actors_in_groups
                    );
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local stats
            use crate::KNOWN_PROCESS_GROUPS;
            use ractor::pg;

            println!("{} {}", "Node:".bright_black(), "local".green());
            println!();

            let registered = registry::registered();
            println!("  {} {}", "Registered Actors:".bold(), registered.len());

            // Count actors by status
            let mut status_counts: HashMap<String, usize> = HashMap::new();
            for name in &registered {
                if let Some(cell) = registry::where_is(name.clone()) {
                    let status = format!("{:?}", cell.get_status());
                    *status_counts.entry(status).or_insert(0) += 1;
                }
            }

            for (status, count) in status_counts {
                println!("    {} {}", status.bright_black(), count);
            }

            println!();

            // Process groups
            let known_groups = KNOWN_PROCESS_GROUPS;
            let mut group_count = 0;
            let mut total_members = 0;

            for group in known_groups {
                let members = pg::get_members(&group.to_string());
                if !members.is_empty() {
                    group_count += 1;
                    total_members += members.len();
                }
            }

            println!("  {} {}", "Process Groups:".bold(), group_count);
            println!("  {} {}", "Actors in Groups:".bold(), total_members);

            println!();

            // Connection info
            if !self.connected_nodes.is_empty() {
                println!(
                    "  {} {}",
                    "Connected Nodes:".bold(),
                    self.connected_nodes.len()
                );
            }

            // Cluster topology info
            if let Some(ref topology) = self.cluster_topology {
                println!(
                    "  {} {} nodes",
                    "Cluster Size:".bold(),
                    topology.nodes.len()
                );
            }
        }

        println!();
        Ok(())
    }
}

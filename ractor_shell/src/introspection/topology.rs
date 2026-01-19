//! Cluster Topology and Supervision Tree Functions
//!
//! This module contains functions for building cluster topology views and supervision trees.

use std::collections::HashMap;

use ractor::rpc::CallResult;
use ractor::{pg, registry, ActorRef};

use crate::protocol::{ActorLocation, ClusterTopology, NodeInfo, ShellProtocolMessage};
use crate::DEFAULT_RPC_TIMEOUT;

use super::INTROSPECTION_GROUP;

/// Build cluster topology by examining process groups and querying remote introspection actors
pub(crate) async fn build_cluster_topology(local_node_name: &str) -> ClusterTopology {
    // Track remote nodes by their node_id AND name (since node_id alone isn't globally unique)
    // Key: node_id, Value: node_name
    let mut remote_nodes: HashMap<u64, String> = HashMap::new();
    let mut process_groups: HashMap<String, Vec<ActorLocation>> = HashMap::new();

    let registered = registry::registered();
    let local_actor_count = registered.len();

    // Query introspection actors in our group to discover remote node names
    // Each remote introspection actor represents a different cluster node
    let introspection_members = pg::get_members(&INTROSPECTION_GROUP.to_string());
    for cell in &introspection_members {
        let actor_id = cell.get_id();

        if actor_id.is_local() {
            // Skip local introspection actor - we know our own name
            continue;
        }

        // Remote introspection actor - query for its node name
        let node_id = actor_id.node();

        // Query remote actors to get their node names
        if let std::collections::hash_map::Entry::Vacant(e) = remote_nodes.entry(node_id) {
            let actor_ref: ActorRef<ShellProtocolMessage> = cell.clone().into();
            if let Ok(CallResult::Success(pong)) = actor_ref
                .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
                .await
            {
                // Parse "pong from {node_name}" to extract node name
                if let Some(name) = pong.strip_prefix("pong from ") {
                    e.insert(name.to_string());
                }
            }
        }
    }

    // Discover all process groups and collect actor locations
    let known_groups = pg::which_groups();

    for group_name in known_groups {
        let members = pg::get_members(&group_name);
        let mut locations = Vec::new();

        for cell in members {
            let actor_id = cell.get_id();

            // Track any new remote nodes we discover
            if !actor_id.is_local() && !remote_nodes.contains_key(&actor_id.node()) {
                // We found a remote actor but don't have its node name yet
                // Use fallback name
                remote_nodes.insert(actor_id.node(), format!("node_{}", actor_id.node()));
            }

            // Get node name for this actor
            let node_name = if actor_id.is_local() {
                local_node_name.to_string()
            } else {
                remote_nodes
                    .get(&actor_id.node())
                    .cloned()
                    .unwrap_or_else(|| format!("node_{}", actor_id.node()))
            };

            locations.push(ActorLocation {
                actor_id: actor_id.to_string(),
                actor_name: cell.get_name(),
                node_id: actor_id.node().to_string(),
                node_name,
            });
        }

        if !locations.is_empty() {
            process_groups.insert(group_name, locations);
        }
    }

    // Build node list: local node + all discovered remote nodes
    let mut nodes: Vec<NodeInfo> = Vec::new();

    // Add local node (always first, ID displayed as "local")
    nodes.push(NodeInfo {
        id: "local".to_string(),
        name: local_node_name.to_string(),
        actor_count: local_actor_count,
        is_local: true,
    });

    // Add remote nodes
    for (node_id, node_name) in &remote_nodes {
        nodes.push(NodeInfo {
            id: node_id.to_string(),
            name: node_name.clone(),
            actor_count: 0, // We don't have remote actor counts
            is_local: false,
        });
    }

    // Sort: local first, then remote nodes by name
    nodes.sort_by(|a, b| match (a.is_local, b.is_local) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    ClusterTopology {
        nodes,
        process_groups,
    }
}

/// Extract node ID from an actor ID string (e.g., "1.0" -> "1")
/// Note: This is kept for backward compatibility and tests. Prefer using ActorId::node() directly.
#[cfg(test)]
pub(crate) fn extract_node_id(actor_id: &str) -> String {
    actor_id.split('.').next().unwrap_or("0").to_string()
}

/// Build supervision trees for all root actors (actors without supervisors)
pub(crate) fn build_supervision_tree_roots() -> Vec<crate::protocol::SupervisionTreeNode> {
    use std::collections::HashSet;

    let mut all_actors = Vec::new();
    let mut seen_ids = HashSet::new();

    // Collect all actors from registry
    for name in registry::registered() {
        if let Some(cell) = registry::where_is(name) {
            let id = cell.get_id();
            if seen_ids.insert(id) {
                all_actors.push(cell);
            }
        }
    }

    // Collect all actors from known process groups
    let known_groups = [
        "ping_pong",
        "ractor_shell_introspection",
        "demo_group",
        "raft_cluster",
    ];
    for group in &known_groups {
        for cell in pg::get_members(&group.to_string()) {
            let id = cell.get_id();
            if seen_ids.insert(id) {
                all_actors.push(cell);
            }
        }
    }

    // Filter to only root actors (those without supervisors)
    let roots: Vec<_> = all_actors
        .iter()
        .filter(|cell| cell.try_get_supervisor().is_none())
        .collect();

    // Build tree nodes for each root
    roots
        .iter()
        .map(|cell| crate::protocol::SupervisionTreeNode::from_cell(cell))
        .collect()
}

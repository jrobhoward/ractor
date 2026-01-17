//! Introspection Actor for Remote Shell Queries
//!
//! This module provides the [`IntrospectionActor`] which enables remote shells to query
//! actor information across distributed nodes.
//!
//! ## Usage
//!
//! When connecting to a remote node, the shell looks for `IntrospectionActor` instances
//! in the well-known process group [`INTROSPECTION_GROUP`]. These actors respond to
//! queries about registered actors, process groups, and cluster topology.
//!
//! ```rust,ignore
//! use ractor::Actor;
//! use ractor_shell::introspection::IntrospectionActor;
//!
//! // Spawn an introspection actor on your node
//! let (actor_ref, _) = Actor::spawn(
//!     Some("introspection".to_string()),
//!     IntrospectionActor,
//!     "my_node".to_string(),
//! ).await?;
//! ```

use crate::dynamic::{supports_dynamic_messages, CallResponse, DynamicMessage};
use crate::protocol::{
    ActorInfo, ActorLocation, ClusterTopology, DynamicCallResult, DynamicSendResult, NodeInfo,
    ShellProtocolMessage,
};
use crate::DEFAULT_RPC_TIMEOUT;
use ractor::rpc::CallResult;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::collections::{HashMap, HashSet};

/// Well-known process group for shell introspection actors
pub const INTROSPECTION_GROUP: &str = "ractor_shell_introspection";

/// Actor that provides introspection capabilities for remote shells.
///
/// This actor responds to [`ShellProtocolMessage`] queries, allowing remote shells
/// to inspect actors, process groups, and cluster topology on this node.
pub struct IntrospectionActor;

/// State for the introspection actor
pub struct IntrospectionState {
    /// Name of this node in the cluster
    pub node_name: String,
}

impl Actor for IntrospectionActor {
    type Msg = ShellProtocolMessage;
    type State = IntrospectionState;
    type Arguments = String; // node name

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        node_name: String,
    ) -> Result<Self::State, ActorProcessingErr> {
        // Join the well-known introspection group
        ractor::pg::join(INTROSPECTION_GROUP.to_string(), vec![myself.get_cell()]);

        Ok(IntrospectionState { node_name })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ShellProtocolMessage::ListRegisteredActors(reply) => {
                let registered = ractor::registry::registered();
                let mut actors = Vec::new();

                for name in registered {
                    if let Some(cell) = ractor::registry::where_is(name) {
                        actors.push(ActorInfo::from_cell(&cell));
                    }
                }

                let _ = reply.send(actors);
            }

            ShellProtocolMessage::GetProcessGroupMembers(group, reply) => {
                let members = ractor::pg::get_members(&group);
                let actors: Vec<ActorInfo> = members.iter().map(ActorInfo::from_cell).collect();

                let _ = reply.send(actors);
            }

            ShellProtocolMessage::GetActorInfo(name, reply) => {
                let info = ractor::registry::where_is(name).map(|cell| ActorInfo::from_cell(&cell));

                let _ = reply.send(info);
            }

            ShellProtocolMessage::StopActor(name) => {
                if let Some(cell) = ractor::registry::where_is(name) {
                    cell.stop(Some("Stopped by remote shell".to_string()));
                }
            }

            ShellProtocolMessage::Ping(reply) => {
                let _ = reply.send(format!("pong from {}", state.node_name));
            }

            ShellProtocolMessage::GetClusterTopology(reply) => {
                let topology = build_cluster_topology(&state.node_name);
                let _ = reply.send(topology);
            }

            ShellProtocolMessage::SendDynamicMessage(actor_name, json_value, reply) => {
                let result = send_dynamic_message_to_actor(&actor_name, json_value).await;
                let _ = reply.send(result);
            }

            ShellProtocolMessage::CallDynamicMessage(actor_name, json_value, reply) => {
                let result = call_dynamic_message_to_actor(&actor_name, json_value).await;
                let _ = reply.send(result);
            }
        }

        Ok(())
    }
}

/// Send a dynamic message to an actor (cast - fire and forget)
async fn send_dynamic_message_to_actor(
    actor_name: &str,
    json_value: serde_json::Value,
) -> DynamicSendResult {
    // Find the actor in registry
    let Some(cell) = ractor::registry::where_is(actor_name.to_string()) else {
        return DynamicSendResult::ActorNotFound;
    };

    // Convert to dynamic message actor reference
    let dynamic_ref: ActorRef<DynamicMessage> = ActorRef::from(cell.clone());

    // Check if the actor supports dynamic messages
    if !supports_dynamic_messages(dynamic_ref.clone()).await {
        return DynamicSendResult::NotDynamic;
    }

    // Send the message
    match dynamic_ref.cast(DynamicMessage::Cast(json_value)) {
        Ok(()) => DynamicSendResult::Success,
        Err(e) => DynamicSendResult::SendFailed(format!("{:?}", e)),
    }
}

/// Call an actor with a dynamic message (RPC - wait for response)
async fn call_dynamic_message_to_actor(
    actor_name: &str,
    json_value: serde_json::Value,
) -> DynamicCallResult {
    // Find the actor in registry
    let Some(cell) = ractor::registry::where_is(actor_name.to_string()) else {
        return DynamicCallResult::ActorNotFound;
    };

    // Convert to dynamic message actor reference
    let dynamic_ref: ActorRef<DynamicMessage> = ActorRef::from(cell.clone());

    // Check if the actor supports dynamic messages
    if !supports_dynamic_messages(dynamic_ref.clone()).await {
        return DynamicCallResult::NotDynamic;
    }

    // Make the RPC call
    let call_result = dynamic_ref
        .call(
            |reply| DynamicMessage::Call(json_value, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match call_result {
        Ok(CallResult::Success(response)) => match response {
            CallResponse::Success(value) => DynamicCallResult::Success(value),
            CallResponse::Error(err) => DynamicCallResult::Error(err),
        },
        Ok(CallResult::Timeout) => {
            DynamicCallResult::CallFailed(format!("Timeout after {:?}", DEFAULT_RPC_TIMEOUT))
        }
        Ok(CallResult::SenderError) => {
            DynamicCallResult::CallFailed("Sender error (actor may have stopped)".to_string())
        }
        Err(e) => DynamicCallResult::CallFailed(format!("{:?}", e)),
    }
}

/// Build cluster topology by examining process groups and actor IDs
fn build_cluster_topology(local_node_name: &str) -> ClusterTopology {
    let mut node_ids: HashSet<String> = HashSet::new();
    let mut node_names: HashMap<String, String> = HashMap::new();
    let mut process_groups: HashMap<String, Vec<ActorLocation>> = HashMap::new();

    // Discover nodes and process groups by examining well-known groups
    // In a real implementation, ractor_cluster would provide a proper API for this
    let known_groups = vec!["ping_pong".to_string(), INTROSPECTION_GROUP.to_string()];

    for group_name in known_groups {
        let members = ractor::pg::get_members(&group_name);
        let mut locations = Vec::new();

        for cell in members {
            let actor_id = cell.get_id();
            let node_id = extract_node_id(&actor_id.to_string());

            node_ids.insert(node_id.clone());

            // Try to extract node name from registered actors
            if let Some(_name) = cell.get_name() {
                // If it's an introspection actor, extract the node name from state
                // For now, we'll use the node_id as a fallback
                if !node_names.contains_key(&node_id) {
                    node_names.insert(node_id.clone(), format!("node_{}", node_id));
                }
            }

            locations.push(ActorLocation {
                actor_id: actor_id.to_string(),
                actor_name: cell.get_name(),
                node_id: node_id.clone(),
                node_name: node_names
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_else(|| format!("node_{}", node_id)),
            });
        }

        if !locations.is_empty() {
            process_groups.insert(group_name, locations);
        }
    }

    // Build node list
    let registered = ractor::registry::registered();
    let local_actor_count = registered.len();

    // Get local node ID from any local actor
    let local_node_id = registered
        .first()
        .and_then(|name| ractor::registry::where_is(name.clone()))
        .map(|cell| extract_node_id(&cell.get_id().to_string()))
        .unwrap_or_else(|| "0".to_string());

    node_ids.insert(local_node_id.clone());

    let mut nodes: Vec<NodeInfo> = node_ids
        .iter()
        .map(|node_id| {
            let is_local = node_id == &local_node_id;
            NodeInfo {
                id: node_id.clone(),
                name: if is_local {
                    local_node_name.to_string()
                } else {
                    node_names
                        .get(node_id)
                        .cloned()
                        .unwrap_or_else(|| format!("node_{}", node_id))
                },
                actor_count: if is_local { local_actor_count } else { 0 },
                is_local,
            }
        })
        .collect();

    // Sort nodes by ID for consistent output
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    ClusterTopology {
        nodes,
        process_groups,
    }
}

/// Extract node ID from an actor ID string (e.g., "1.0" -> "1")
fn extract_node_id(actor_id: &str) -> String {
    actor_id.split('.').next().unwrap_or("0").to_string()
}

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
    ShellProtocolMessage, TypedRpcResult,
};
use crate::DEFAULT_RPC_TIMEOUT;
use std::collections::{HashMap, HashSet};

use ractor::rpc::CallResult;
use ractor::{pg, registry, Actor, ActorProcessingErr, ActorRef};

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
        pg::join(INTROSPECTION_GROUP.to_string(), vec![myself.get_cell()]);

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
                let registered = registry::registered();
                let mut actors = Vec::new();

                for name in registered {
                    if let Some(cell) = registry::where_is(name) {
                        actors.push(ActorInfo::from_cell(&cell));
                    }
                }

                let _ = reply.send(actors);
            }

            ShellProtocolMessage::GetProcessGroupMembers(group, reply) => {
                let members = pg::get_members(&group);
                let actors: Vec<ActorInfo> = members.iter().map(ActorInfo::from_cell).collect();

                let _ = reply.send(actors);
            }

            ShellProtocolMessage::GetActorInfo(name, reply) => {
                let info = registry::where_is(name).map(|cell| ActorInfo::from_cell(&cell));

                let _ = reply.send(info);
            }

            ShellProtocolMessage::StopActor(name) => {
                if let Some(cell) = registry::where_is(name) {
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

            // ==================== Schema Introspection ====================
            ShellProtocolMessage::GetMessageSchema(actor_name, reply) => {
                let schema = crate::schema_registry::get_schema(&actor_name);
                let _ = reply.send(schema);
            }

            ShellProtocolMessage::ListSchemaActors(reply) => {
                let schemas = crate::schema_registry::list_schemas();
                let schema_actors: Vec<crate::protocol::SchemaActorInfo> = schemas
                    .into_iter()
                    .map(|(name, schema)| crate::protocol::SchemaActorInfo { name, schema })
                    .collect();
                let _ = reply.send(schema_actors);
            }

            // ==================== Typed RPC ====================
            ShellProtocolMessage::CallTypedRpc(actor_name, variant_name, args, reply) => {
                let result = call_typed_rpc_on_actor(&actor_name, &variant_name, args).await;
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
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
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
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
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

/// Call a typed RPC on a schema-enabled actor
async fn call_typed_rpc_on_actor(
    actor_name: &str,
    variant_name: &str,
    _args: serde_json::Value,
) -> TypedRpcResult {
    // Check if the actor has a registered schema
    if !crate::schema_registry::has_schema(actor_name) {
        return TypedRpcResult::NotSchemaEnabled;
    }

    // Find the actor in registry
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return TypedRpcResult::ActorNotFound;
    };

    // Handle raft_node specifically (it's the main typed actor currently)
    if actor_name == "raft_node" {
        use crate::raft::RaftMessage;

        let raft_ref: ActorRef<RaftMessage> = ActorRef::from(cell);

        match variant_name {
            "GetStatus" => {
                let call_result = raft_ref
                    .call(RaftMessage::GetStatus, Some(DEFAULT_RPC_TIMEOUT))
                    .await;

                match call_result {
                    Ok(CallResult::Success(status)) => {
                        let json = serde_json::to_value(&status).unwrap_or(serde_json::Value::Null);
                        TypedRpcResult::Success(json)
                    }
                    Ok(CallResult::Timeout) => TypedRpcResult::CallFailed(format!(
                        "Timeout after {:?}",
                        DEFAULT_RPC_TIMEOUT
                    )),
                    Ok(CallResult::SenderError) => TypedRpcResult::CallFailed(
                        "Sender error (actor may have stopped)".to_string(),
                    ),
                    Err(e) => TypedRpcResult::CallFailed(format!("{:?}", e)),
                }
            }
            "IsLeader" => {
                let call_result = raft_ref
                    .call(RaftMessage::IsLeader, Some(DEFAULT_RPC_TIMEOUT))
                    .await;

                match call_result {
                    Ok(CallResult::Success(is_leader)) => {
                        TypedRpcResult::Success(serde_json::Value::Bool(is_leader))
                    }
                    Ok(CallResult::Timeout) => TypedRpcResult::CallFailed(format!(
                        "Timeout after {:?}",
                        DEFAULT_RPC_TIMEOUT
                    )),
                    Ok(CallResult::SenderError) => TypedRpcResult::CallFailed(
                        "Sender error (actor may have stopped)".to_string(),
                    ),
                    Err(e) => TypedRpcResult::CallFailed(format!("{:?}", e)),
                }
            }
            "GetLeader" => {
                let call_result = raft_ref
                    .call(RaftMessage::GetLeader, Some(DEFAULT_RPC_TIMEOUT))
                    .await;

                match call_result {
                    Ok(CallResult::Success(leader)) => {
                        let json = leader
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null);
                        TypedRpcResult::Success(json)
                    }
                    Ok(CallResult::Timeout) => TypedRpcResult::CallFailed(format!(
                        "Timeout after {:?}",
                        DEFAULT_RPC_TIMEOUT
                    )),
                    Ok(CallResult::SenderError) => TypedRpcResult::CallFailed(
                        "Sender error (actor may have stopped)".to_string(),
                    ),
                    Err(e) => TypedRpcResult::CallFailed(format!("{:?}", e)),
                }
            }
            "GetPeers" => {
                let call_result = raft_ref
                    .call(RaftMessage::GetPeers, Some(DEFAULT_RPC_TIMEOUT))
                    .await;

                match call_result {
                    Ok(CallResult::Success(peers)) => {
                        let json = serde_json::to_value(&peers).unwrap_or(serde_json::Value::Null);
                        TypedRpcResult::Success(json)
                    }
                    Ok(CallResult::Timeout) => TypedRpcResult::CallFailed(format!(
                        "Timeout after {:?}",
                        DEFAULT_RPC_TIMEOUT
                    )),
                    Ok(CallResult::SenderError) => TypedRpcResult::CallFailed(
                        "Sender error (actor may have stopped)".to_string(),
                    ),
                    Err(e) => TypedRpcResult::CallFailed(format!("{:?}", e)),
                }
            }
            _ => TypedRpcResult::UnknownVariant(variant_name.to_string()),
        }
    } else {
        // For other schema-enabled actors, we would need a generic way to dispatch
        // For now, return NotSchemaEnabled since we only have raft_node
        TypedRpcResult::NotSchemaEnabled
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
        let members = pg::get_members(&group_name);
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
    let registered = registry::registered();
    let local_actor_count = registered.len();

    // Get local node ID from any local actor
    let local_node_id = registered
        .first()
        .and_then(|name| registry::where_is(name.clone()))
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

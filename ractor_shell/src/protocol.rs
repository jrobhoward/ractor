//! Shell Protocol Messages
//!
//! Defines the message types used for communication between the shell and remote
//! introspection actors. These messages are designed to work over `ractor_cluster`
//! network connections.
//!
//! ## Message Types
//!
//! - [`ShellProtocolMessage`]: RPC messages for querying remote nodes
//! - [`ActorInfo`]: Serializable actor metadata
//! - [`ClusterTopology`]: Cluster-wide topology information
//! - [`NodeInfo`]: Information about a single node
//! - [`ActorLocation`]: Actor location within the cluster

use ractor::RpcReplyPort;
use ractor_cluster::RactorClusterMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a dynamic message send operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DynamicSendResult {
    /// Message was sent successfully
    Success,
    /// Actor was not found
    ActorNotFound,
    /// Actor doesn't support dynamic messages
    NotDynamic,
    /// Failed to send the message
    SendFailed(String),
}

/// Result of a dynamic message call operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DynamicCallResult {
    /// Call completed successfully with a response
    Success(serde_json::Value),
    /// Call completed with an error response from the actor
    Error(String),
    /// Actor was not found
    ActorNotFound,
    /// Actor doesn't support dynamic messages
    NotDynamic,
    /// Call failed (e.g., timeout, channel error)
    CallFailed(String),
}

/// Messages for shell protocol communication with remote nodes
#[derive(RactorClusterMessage, Debug)]
#[ractor_shell]
pub enum ShellProtocolMessage {
    /// List all registered actors on the target node
    #[rpc]
    ListRegisteredActors(RpcReplyPort<Vec<ActorInfo>>),

    /// Get process group members
    #[rpc]
    GetProcessGroupMembers(String, RpcReplyPort<Vec<ActorInfo>>),

    /// Get detailed info about a specific actor
    #[rpc]
    GetActorInfo(String, RpcReplyPort<Option<ActorInfo>>),

    /// Stop an actor by name
    StopActor(String),

    /// Ping to check connectivity
    #[rpc]
    Ping(RpcReplyPort<String>),

    /// Get cluster topology from this node's perspective
    #[rpc]
    GetClusterTopology(RpcReplyPort<ClusterTopology>),

    /// Send a dynamic message to an actor (cast - fire and forget)
    #[rpc]
    SendDynamicMessage(String, serde_json::Value, RpcReplyPort<DynamicSendResult>),

    /// Call an actor with a dynamic message (RPC - wait for response)
    #[rpc]
    CallDynamicMessage(String, serde_json::Value, RpcReplyPort<DynamicCallResult>),

    // ==================== Schema Introspection ====================
    /// Get the message schema for an actor (returns None if not schema-enabled)
    #[rpc]
    GetMessageSchema(String, RpcReplyPort<Option<String>>),

    /// List all actors with registered schemas
    #[rpc]
    ListSchemaActors(RpcReplyPort<Vec<SchemaActorInfo>>),

    // ==================== Typed RPC ====================
    /// Call a typed RPC on a schema-enabled actor (actor_name, variant_name, args_json)
    #[rpc]
    CallTypedRpc(
        String,
        String,
        serde_json::Value,
        RpcReplyPort<TypedRpcResult>,
    ),
}

/// Result of a typed RPC call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypedRpcResult {
    /// Call completed successfully with JSON response
    Success(serde_json::Value),
    /// Actor was not found
    ActorNotFound,
    /// Actor doesn't have a registered schema
    NotSchemaEnabled,
    /// Unknown RPC variant
    UnknownVariant(String),
    /// Call failed (e.g., timeout, channel error)
    CallFailed(String),
}

/// Information about an actor (serializable across network)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorInfo {
    /// Actor's unique identifier
    pub id: String,
    /// Actor's registered name, if any
    pub name: Option<String>,
    /// Current actor status (e.g., "Running", "Stopped")
    pub status: String,
    /// Whether this actor is on the local node
    pub is_local: bool,
}

impl ActorInfo {
    /// Create ActorInfo from an ActorCell reference
    pub fn from_cell(cell: &ractor::ActorCell) -> Self {
        Self {
            id: cell.get_id().to_string(),
            name: cell.get_name(),
            status: format!("{:?}", cell.get_status()),
            is_local: cell.get_id().is_local(),
        }
    }
}

/// Cluster topology information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterTopology {
    /// All known nodes in the cluster
    pub nodes: Vec<NodeInfo>,
    /// Process groups and their members across the cluster
    pub process_groups: HashMap<String, Vec<ActorLocation>>,
}

/// Information about a node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node ID (e.g., "0", "1", "2")
    pub id: String,
    /// Node name (e.g., "node_a", "node_b")
    pub name: String,
    /// Number of registered actors on this node
    pub actor_count: usize,
    /// Whether this is the local node
    pub is_local: bool,
}

/// Location of an actor in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorLocation {
    /// Actor's unique identifier
    pub actor_id: String,
    /// Actor's registered name, if any
    pub actor_name: Option<String>,
    /// ID of the node hosting this actor
    pub node_id: String,
    /// Name of the node hosting this actor
    pub node_name: String,
}

/// Information about a schema-enabled actor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaActorInfo {
    /// Actor's registered name
    pub name: String,
    /// The JSON schema string describing the message type
    pub schema: String,
}

#[cfg(test)]
mod protocol_tests;

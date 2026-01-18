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

    /// List all process groups on the target node
    #[rpc]
    ListProcessGroups(RpcReplyPort<Vec<String>>),

    /// Get process group members
    #[rpc]
    GetProcessGroupMembers(String, RpcReplyPort<Vec<ActorInfo>>),

    /// Get all process groups with their members (for tree display)
    #[rpc]
    GetProcessGroupTree(RpcReplyPort<HashMap<String, Vec<ActorInfo>>>),

    /// Get detailed info about a specific actor
    #[rpc]
    GetActorInfo(String, RpcReplyPort<Option<ActorInfo>>),

    /// Stop an actor by name (returns true if actor was found and stop signal sent)
    #[rpc]
    StopActor(String, RpcReplyPort<bool>),

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

    // ==================== Remote Tracing ====================
    /// Subscribe to trace events from this node (pattern, returns subscription ID)
    #[rpc]
    SubscribeToTraces(String, RpcReplyPort<SubscriptionId>),

    /// Poll for trace events from a subscription (subscription_id)
    #[rpc]
    PollTraces(SubscriptionId, RpcReplyPort<TraceEventBatch>),

    /// Unsubscribe from trace events
    #[rpc]
    UnsubscribeFromTraces(SubscriptionId, RpcReplyPort<bool>),

    /// Internal: Receive a trace event from the local tracing layer (cast message)
    /// This is not used by remote clients, only by the local tracing infrastructure
    TraceEventNotification(SerializableTraceEvent),

    // ==================== Supervision Tree ====================
    /// Get the supervision tree starting from an optional actor (None = all roots)
    #[rpc]
    GetSupervisionTree(Option<String>, RpcReplyPort<Vec<SupervisionTreeNode>>),

    /// Get the parent (supervisor) of an actor
    #[rpc]
    GetActorParent(String, RpcReplyPort<Option<ActorInfo>>),
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

/// Unique identifier for a trace subscription
pub type SubscriptionId = u64;

/// Batch of trace events with dropped count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEventBatch {
    /// Trace events since last poll
    pub events: Vec<SerializableTraceEvent>,
    /// Number of events dropped due to buffer overflow since last poll
    pub dropped_count: usize,
}

/// Serializable trace event for network transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableTraceEvent {
    /// Timestamp as RFC3339 string
    pub timestamp: String,
    /// Actor ID (e.g., "0.1")
    pub actor_id: Option<String>,
    /// Actor name (if registered)
    pub actor_name: Option<String>,
    /// Event type (span enter, exit, or event)
    pub event_type: String,
    /// Event level (trace, debug, info, warn, error)
    pub level: String,
    /// Span/event target
    pub target: String,
    /// Event message or span name
    pub message: String,
    /// Additional fields
    pub fields: Vec<(String, String)>,
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

/// Supervision tree node (serializable for remote queries)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionTreeNode {
    /// Information about this actor
    pub actor: ActorInfo,
    /// Number of children
    pub child_count: usize,
    /// Child actors in the supervision tree
    pub children: Vec<SupervisionTreeNode>,
}

impl SupervisionTreeNode {
    /// Build a supervision tree starting from an ActorCell
    pub fn from_cell(cell: &ractor::ActorCell) -> Self {
        let children_cells = cell.get_children();
        let children: Vec<SupervisionTreeNode> = children_cells
            .iter()
            .map(SupervisionTreeNode::from_cell)
            .collect();

        Self {
            actor: ActorInfo::from_cell(cell),
            child_count: children.len(),
            children,
        }
    }
}

impl SerializableTraceEvent {
    /// Convert from internal TraceEvent to serializable format
    pub fn from_trace_event(event: &crate::tracing::TraceEvent) -> Self {
        Self {
            timestamp: event.timestamp.to_rfc3339(),
            actor_id: event.actor_id.clone(),
            actor_name: event.actor_name.clone(),
            event_type: format!("{}", event.event_type),
            level: format!("{}", event.level),
            target: event.target.clone(),
            message: event.message.clone(),
            fields: event.fields.clone(),
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

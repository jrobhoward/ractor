use ractor::RpcReplyPort;
use ractor_cluster::RactorClusterMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Messages for shell protocol communication with remote nodes
#[derive(RactorClusterMessage, Debug)]
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
}

/// Information about an actor (serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorInfo {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub is_local: bool,
}

impl ActorInfo {
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
    pub actor_id: String,
    pub actor_name: Option<String>,
    pub node_id: String,
    pub node_name: String,
}

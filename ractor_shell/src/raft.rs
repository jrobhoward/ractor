//! Raft-based leader election for cluster nodes.
//!
//! This module implements a simplified Raft consensus algorithm focused on leader election.
//! It provides actors that participate in leader election and can report their status
//! via DynamicMessage for shell interaction.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ractor::Actor;
//! use ractor_shell::raft::{RaftNode, RaftConfig};
//!
//! // Create a Raft node that participates in leader election
//! let config = RaftConfig {
//!     node_name: "node1".to_string(),
//!     ..Default::default()
//! };
//!
//! let (raft_ref, _) = Actor::spawn(
//!     Some("raft_node".to_string()),
//!     RaftNode,
//!     config,
//! ).await?;
//! ```
//!
//! ## Shell Commands
//!
//! Once running, you can query the Raft node via the shell:
//!
//! ```text
//! ractor@local > call raft_node {"command": "is_leader"}
//! {"is_leader": false, "node_name": "node1", "term": 1}
//!
//! ractor@local > call raft_node {"command": "get_leader"}
//! {"leader": "node2", "term": 1}
//!
//! ractor@local > call raft_node {"command": "status"}
//! {"node_name": "node1", "role": "Follower", "term": 1, "leader": "node2", "peers": 2}
//! ```

use std::collections::HashMap;
use std::time::Duration;

use ractor::rpc::CallResult;
use ractor::{pg, Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};

use crate::dynamic::{CallResponse, DynamicMessage};
use crate::introspection::INTROSPECTION_GROUP;
use crate::protocol::ShellProtocolMessage;

// ==================== Constants ====================

/// Well-known process group for Raft cluster members
pub const RAFT_CLUSTER_GROUP: &str = "raft_cluster";

/// Default election timeout range (ms) - longer for network latency
pub const DEFAULT_ELECTION_TIMEOUT_MIN_MS: u64 = 1000;
pub const DEFAULT_ELECTION_TIMEOUT_MAX_MS: u64 = 2000;

/// Default heartbeat interval (ms) - must be less than election timeout
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 300;

/// Peer discovery interval (ms)
pub const PEER_DISCOVERY_INTERVAL_MS: u64 = 2000;

// ==================== Internal Protocol Messages ====================

/// Internal message types encoded as JSON for Raft protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "raft_type")]
pub enum RaftProtocol {
    /// Election timeout fired (internal) - includes generation to detect stale timers
    ElectionTimeout { generation: u64 },
    /// Heartbeat timeout fired (internal) - includes generation
    HeartbeatTimeout { generation: u64 },
    /// Peer discovery timeout (internal) - includes generation
    DiscoverPeers { generation: u64 },
    /// Request vote from this node
    RequestVote { term: u64, candidate_name: String },
    /// Vote response
    VoteResponse {
        term: u64,
        vote_granted: bool,
        voter_name: String,
    },
    /// Heartbeat from leader
    Heartbeat { term: u64, leader_name: String },
}

// ==================== Configuration ====================

/// Configuration for a Raft node
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// Name of this node in the cluster
    pub node_name: String,
    /// Minimum election timeout in milliseconds
    pub election_timeout_min_ms: u64,
    /// Maximum election timeout in milliseconds
    pub election_timeout_max_ms: u64,
    /// Heartbeat interval in milliseconds (leader only)
    pub heartbeat_interval_ms: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_name: "node".to_string(),
            election_timeout_min_ms: DEFAULT_ELECTION_TIMEOUT_MIN_MS,
            election_timeout_max_ms: DEFAULT_ELECTION_TIMEOUT_MAX_MS,
            heartbeat_interval_ms: DEFAULT_HEARTBEAT_INTERVAL_MS,
        }
    }
}

// ==================== Raft State ====================

/// The role of a Raft node in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftRole {
    /// Following a leader, waiting for heartbeats
    Follower,
    /// Requesting votes to become leader
    Candidate,
    /// Currently the elected leader
    Leader,
}

impl std::fmt::Display for RaftRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaftRole::Follower => write!(f, "Follower"),
            RaftRole::Candidate => write!(f, "Candidate"),
            RaftRole::Leader => write!(f, "Leader"),
        }
    }
}

/// Information about a remote peer (accessed via introspection)
#[derive(Debug, Clone)]
struct PeerInfo {
    /// Reference to the peer's introspection actor
    introspection_ref: ActorRef<ShellProtocolMessage>,
}

/// State for a Raft node
pub struct RaftState {
    /// Configuration
    pub config: RaftConfig,
    /// Current role
    pub role: RaftRole,
    /// Current term number (increases on each election)
    pub current_term: u64,
    /// Who we voted for in the current term
    pub voted_for: Option<String>,
    /// Current leader (if known)
    pub current_leader: Option<String>,
    /// Known peers in the cluster (node_name -> PeerInfo)
    peers: HashMap<String, PeerInfo>,
    /// Votes received in current election (when Candidate)
    pub votes_received: Vec<String>,
    /// Generation counter for election timers (to ignore stale timers)
    election_timer_generation: u64,
    /// Generation counter for heartbeat timers
    heartbeat_timer_generation: u64,
    /// Generation counter for peer discovery timers
    discovery_timer_generation: u64,
}

impl RaftState {
    pub fn new(config: RaftConfig) -> Self {
        Self {
            config,
            role: RaftRole::Follower,
            current_term: 0,
            voted_for: None,
            current_leader: None,
            peers: HashMap::new(),
            votes_received: Vec::new(),
            election_timer_generation: 0,
            heartbeat_timer_generation: 0,
            discovery_timer_generation: 0,
        }
    }

    pub fn is_leader(&self) -> bool {
        self.role == RaftRole::Leader
    }

    fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Calculate a randomized election timeout
    pub fn election_timeout(&self) -> Duration {
        use rand::Rng;
        let mut rng = rand::rng();
        let timeout_ms = rng
            .random_range(self.config.election_timeout_min_ms..self.config.election_timeout_max_ms);
        Duration::from_millis(timeout_ms)
    }

    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.config.heartbeat_interval_ms)
    }

    /// Increment election timer generation and return the new value
    fn next_election_generation(&mut self) -> u64 {
        self.election_timer_generation += 1;
        self.election_timer_generation
    }

    /// Increment heartbeat timer generation and return the new value
    fn next_heartbeat_generation(&mut self) -> u64 {
        self.heartbeat_timer_generation += 1;
        self.heartbeat_timer_generation
    }

    /// Increment discovery timer generation and return the new value
    fn next_discovery_generation(&mut self) -> u64 {
        self.discovery_timer_generation += 1;
        self.discovery_timer_generation
    }
}

/// Response to a vote request (kept for test compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    /// Current term of the voter
    pub term: u64,
    /// Whether vote was granted
    pub vote_granted: bool,
    /// Name of the voter
    pub voter_name: String,
}

// ==================== Raft Node Actor ====================

/// Actor that participates in Raft leader election.
///
/// This actor uses DynamicMessage as its message type for shell compatibility.
/// Raft protocol messages are encoded as JSON and sent via remote introspection actors.
pub struct RaftNode;

impl Actor for RaftNode {
    type Msg = DynamicMessage;
    type State = RaftState;
    type Arguments = RaftConfig;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        config: RaftConfig,
    ) -> Result<Self::State, ActorProcessingErr> {
        // Join the Raft cluster process group (for local discovery)
        pg::join(RAFT_CLUSTER_GROUP.to_string(), vec![myself.get_cell()]);

        let mut state = RaftState::new(config);

        // Schedule initial peer discovery
        let gen = state.next_discovery_generation();
        myself.send_after(Duration::from_millis(500), move || {
            DynamicMessage::Cast(
                serde_json::to_value(&RaftProtocol::DiscoverPeers { generation: gen }).unwrap(),
            )
        });

        // Schedule initial election timeout
        let timeout = state.election_timeout();
        let gen = state.next_election_generation();
        myself.send_after(timeout, move || {
            DynamicMessage::Cast(
                serde_json::to_value(&RaftProtocol::ElectionTimeout { generation: gen }).unwrap(),
            )
        });

        tracing::info!(
            node = %state.config.node_name,
            "RaftNode started, waiting for peer discovery"
        );

        Ok(state)
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DynamicMessage::Ping(reply) => {
                let _ = reply.send(true);
            }
            DynamicMessage::Cast(json) => {
                self.handle_cast(myself, state, json).await?;
            }
            DynamicMessage::Call(json, reply) => {
                let response = self.handle_call_command(state, json);
                let _ = reply.send(response);
            }
        }

        Ok(())
    }
}

impl RaftNode {
    /// Handle a cast message (internal protocol or external command)
    async fn handle_cast(
        &self,
        myself: ActorRef<DynamicMessage>,
        state: &mut RaftState,
        json: serde_json::Value,
    ) -> Result<(), ActorProcessingErr> {
        // Try to parse as internal Raft protocol message
        if let Ok(protocol) = serde_json::from_value::<RaftProtocol>(json.clone()) {
            match protocol {
                RaftProtocol::DiscoverPeers { generation } => {
                    // Only process if generation matches (ignore stale timers)
                    if generation == state.discovery_timer_generation {
                        self.discover_peers(myself.clone(), state).await?;
                    }
                }
                RaftProtocol::ElectionTimeout { generation } => {
                    // Only process if generation matches (ignore stale timers)
                    if generation == state.election_timer_generation {
                        self.handle_election_timeout(myself, state).await?;
                    }
                }
                RaftProtocol::HeartbeatTimeout { generation } => {
                    // Only process if generation matches (ignore stale timers)
                    if generation == state.heartbeat_timer_generation {
                        self.handle_heartbeat_timeout(myself, state).await?;
                    }
                }
                RaftProtocol::RequestVote {
                    term,
                    candidate_name,
                } => {
                    self.handle_request_vote(myself, state, term, candidate_name)
                        .await?;
                }
                RaftProtocol::VoteResponse {
                    term,
                    vote_granted,
                    voter_name,
                } => {
                    self.handle_vote_response(myself, state, term, vote_granted, voter_name)
                        .await?;
                }
                RaftProtocol::Heartbeat { term, leader_name } => {
                    self.handle_heartbeat(myself, state, term, leader_name)
                        .await?;
                }
            }
        }
        // Silently ignore unknown messages (could be from shell)

        Ok(())
    }

    /// Discover peers by finding remote introspection actors
    async fn discover_peers(
        &self,
        myself: ActorRef<DynamicMessage>,
        state: &mut RaftState,
    ) -> Result<(), ActorProcessingErr> {
        // Find all introspection actors in the well-known group
        let members = pg::get_members(&INTROSPECTION_GROUP.to_string());

        // Filter for remote introspection actors
        let remote_introspection: Vec<ActorRef<ShellProtocolMessage>> = members
            .into_iter()
            .filter(|cell| !cell.get_id().is_local())
            .map(ActorRef::<ShellProtocolMessage>::from)
            .collect();

        let discovered_count = remote_introspection.len();

        // For each remote introspection actor, ping to get node name
        for introspection_ref in remote_introspection {
            // Skip if we already know this peer by actor ID
            let actor_id = introspection_ref.get_id().to_string();
            if state
                .peers
                .values()
                .any(|p| p.introspection_ref.get_id().to_string() == actor_id)
            {
                continue;
            }

            // Ping to get node name
            match introspection_ref
                .call(ShellProtocolMessage::Ping, Some(Duration::from_millis(500)))
                .await
            {
                Ok(CallResult::Success(pong)) => {
                    // Extract node name from "pong from {node_name}"
                    let node_name = pong.strip_prefix("pong from ").unwrap_or(&pong).to_string();

                    if node_name != state.config.node_name && !state.peers.contains_key(&node_name)
                    {
                        tracing::info!(
                            local = %state.config.node_name,
                            peer = %node_name,
                            "Discovered Raft peer"
                        );

                        state
                            .peers
                            .insert(node_name.clone(), PeerInfo { introspection_ref });
                    }
                }
                _ => {
                    // Failed to ping, skip this introspection actor
                }
            }
        }

        if discovered_count > 0 && state.peers.is_empty() {
            tracing::debug!(
                node = %state.config.node_name,
                remote_introspection_count = discovered_count,
                "Found remote introspection actors but no new peers"
            );
        }

        // Schedule next peer discovery (with new generation to invalidate any pending)
        let gen = state.next_discovery_generation();
        myself.send_after(
            Duration::from_millis(PEER_DISCOVERY_INTERVAL_MS),
            move || {
                DynamicMessage::Cast(
                    serde_json::to_value(&RaftProtocol::DiscoverPeers { generation: gen }).unwrap(),
                )
            },
        );

        Ok(())
    }

    /// Send a Raft protocol message to all peers via their introspection actors
    async fn broadcast_to_peers(&self, state: &RaftState, message: &RaftProtocol) {
        let json = serde_json::to_value(message).unwrap();

        for (_name, peer) in &state.peers {
            // Send via introspection's SendDynamicMessage (fire and forget via spawned task)
            let introspection_ref = peer.introspection_ref.clone();
            let json_clone = json.clone();
            tokio::spawn(async move {
                let _ = introspection_ref
                    .call(
                        |reply| {
                            ShellProtocolMessage::SendDynamicMessage(
                                "raft_node".to_string(),
                                json_clone,
                                reply,
                            )
                        },
                        Some(Duration::from_millis(200)),
                    )
                    .await;
            });
        }
    }

    /// Send a Raft protocol message to a specific peer
    async fn send_to_peer(&self, state: &RaftState, peer_name: &str, message: &RaftProtocol) {
        if let Some(peer) = state.peers.get(peer_name) {
            let json = serde_json::to_value(message).unwrap();
            let introspection_ref = peer.introspection_ref.clone();
            tokio::spawn(async move {
                let _ = introspection_ref
                    .call(
                        |reply| {
                            ShellProtocolMessage::SendDynamicMessage(
                                "raft_node".to_string(),
                                json,
                                reply,
                            )
                        },
                        Some(Duration::from_millis(200)),
                    )
                    .await;
            });
        }
    }

    /// Schedule a new election timeout (invalidates any pending election timers)
    fn schedule_election_timeout(myself: &ActorRef<DynamicMessage>, state: &mut RaftState) {
        let timeout = state.election_timeout();
        let gen = state.next_election_generation();
        myself.send_after(timeout, move || {
            DynamicMessage::Cast(
                serde_json::to_value(&RaftProtocol::ElectionTimeout { generation: gen }).unwrap(),
            )
        });
    }

    /// Schedule a new heartbeat timeout (invalidates any pending heartbeat timers)
    fn schedule_heartbeat_timeout(myself: &ActorRef<DynamicMessage>, state: &mut RaftState) {
        let interval = state.heartbeat_interval();
        let gen = state.next_heartbeat_generation();
        myself.send_after(interval, move || {
            DynamicMessage::Cast(
                serde_json::to_value(&RaftProtocol::HeartbeatTimeout { generation: gen }).unwrap(),
            )
        });
    }

    /// Handle election timeout - transition to candidate and request votes
    async fn handle_election_timeout(
        &self,
        myself: ActorRef<DynamicMessage>,
        state: &mut RaftState,
    ) -> Result<(), ActorProcessingErr> {
        // Don't start election if we're already leader
        if state.role == RaftRole::Leader {
            return Ok(());
        }

        // Don't start election if we have no peers yet
        if state.peers.is_empty() {
            tracing::debug!(
                node = %state.config.node_name,
                "Skipping election - no peers discovered yet"
            );
            Self::schedule_election_timeout(&myself, state);
            return Ok(());
        }

        // Transition to candidate
        state.role = RaftRole::Candidate;
        state.current_term += 1;
        state.voted_for = Some(state.config.node_name.clone());
        state.votes_received.clear();
        state.votes_received.push(state.config.node_name.clone()); // Vote for self
        state.current_leader = None;

        tracing::info!(
            node = %state.config.node_name,
            term = state.current_term,
            peers = state.peers.len(),
            "Starting election"
        );

        // Request votes from all peers
        let vote_request = RaftProtocol::RequestVote {
            term: state.current_term,
            candidate_name: state.config.node_name.clone(),
        };
        self.broadcast_to_peers(state, &vote_request).await;

        // Check if we already have majority (single node cluster)
        self.check_election_result(&myself, state).await?;

        // Schedule next election timeout (with new generation)
        Self::schedule_election_timeout(&myself, state);

        Ok(())
    }

    /// Handle vote request from a candidate
    async fn handle_request_vote(
        &self,
        myself: ActorRef<DynamicMessage>,
        state: &mut RaftState,
        term: u64,
        candidate_name: String,
    ) -> Result<(), ActorProcessingErr> {
        let mut vote_granted = false;

        // If candidate's term is higher, update our term and become follower
        if term > state.current_term {
            state.current_term = term;
            state.role = RaftRole::Follower;
            state.voted_for = None;
            state.current_leader = None;
        }

        // Grant vote if:
        // 1. Term is at least as high as ours
        // 2. We haven't voted in this term OR we already voted for this candidate
        if term >= state.current_term
            && (state.voted_for.is_none() || state.voted_for.as_ref() == Some(&candidate_name))
        {
            state.voted_for = Some(candidate_name.clone());
            vote_granted = true;

            tracing::debug!(
                voter = %state.config.node_name,
                candidate = %candidate_name,
                term = term,
                "Granted vote"
            );

            // Reset election timeout since we voted
            Self::schedule_election_timeout(&myself, state);
        }

        // Send vote response back to the candidate
        let response = RaftProtocol::VoteResponse {
            term: state.current_term,
            vote_granted,
            voter_name: state.config.node_name.clone(),
        };
        self.send_to_peer(state, &candidate_name, &response).await;

        Ok(())
    }

    /// Handle vote response
    async fn handle_vote_response(
        &self,
        myself: ActorRef<DynamicMessage>,
        state: &mut RaftState,
        term: u64,
        vote_granted: bool,
        voter_name: String,
    ) -> Result<(), ActorProcessingErr> {
        // If we see a higher term, become follower
        if term > state.current_term {
            state.current_term = term;
            state.role = RaftRole::Follower;
            state.voted_for = None;
            state.current_leader = None;
            Self::schedule_election_timeout(&myself, state);
            return Ok(());
        }

        // Ignore if we're no longer a candidate or term doesn't match
        if state.role != RaftRole::Candidate || term != state.current_term {
            return Ok(());
        }

        if vote_granted && !state.votes_received.contains(&voter_name) {
            tracing::debug!(
                node = %state.config.node_name,
                voter = %voter_name,
                term = term,
                "Received vote"
            );
            state.votes_received.push(voter_name);
            self.check_election_result(&myself, state).await?;
        }

        Ok(())
    }

    /// Check if we have enough votes to become leader
    async fn check_election_result(
        &self,
        myself: &ActorRef<DynamicMessage>,
        state: &mut RaftState,
    ) -> Result<(), ActorProcessingErr> {
        if state.role != RaftRole::Candidate {
            return Ok(());
        }

        let total_nodes = state.peers.len() + 1; // +1 for self
        let votes_needed = (total_nodes / 2) + 1;

        if state.votes_received.len() >= votes_needed {
            // Become leader!
            state.role = RaftRole::Leader;
            state.current_leader = Some(state.config.node_name.clone());

            tracing::info!(
                node = %state.config.node_name,
                term = state.current_term,
                votes = state.votes_received.len(),
                total = total_nodes,
                "Elected as leader"
            );

            // Start sending heartbeats immediately
            self.send_heartbeats(state).await?;

            // Schedule heartbeat timer (with new generation)
            Self::schedule_heartbeat_timeout(myself, state);
        }

        Ok(())
    }

    /// Handle heartbeat timeout - send heartbeats to all peers
    async fn handle_heartbeat_timeout(
        &self,
        myself: ActorRef<DynamicMessage>,
        state: &mut RaftState,
    ) -> Result<(), ActorProcessingErr> {
        if state.role != RaftRole::Leader {
            return Ok(());
        }

        self.send_heartbeats(state).await?;

        // Schedule next heartbeat (with new generation)
        Self::schedule_heartbeat_timeout(&myself, state);

        Ok(())
    }

    /// Send heartbeats to all peers
    async fn send_heartbeats(&self, state: &RaftState) -> Result<(), ActorProcessingErr> {
        let heartbeat = RaftProtocol::Heartbeat {
            term: state.current_term,
            leader_name: state.config.node_name.clone(),
        };
        self.broadcast_to_peers(state, &heartbeat).await;
        Ok(())
    }

    /// Handle heartbeat from leader
    async fn handle_heartbeat(
        &self,
        myself: ActorRef<DynamicMessage>,
        state: &mut RaftState,
        term: u64,
        leader_name: String,
    ) -> Result<(), ActorProcessingErr> {
        // If leader's term is at least as high as ours, accept them as leader
        if term >= state.current_term {
            let role_changed = state.role != RaftRole::Follower;
            let leader_changed = state.current_leader.as_ref() != Some(&leader_name);

            if role_changed || leader_changed || term > state.current_term {
                tracing::info!(
                    node = %state.config.node_name,
                    leader = %leader_name,
                    term = term,
                    old_role = %state.role,
                    "Accepting leader"
                );
            }

            state.current_term = term;
            state.role = RaftRole::Follower;
            state.current_leader = Some(leader_name);
            state.voted_for = None;

            // Reset election timeout since we heard from leader (with new generation)
            Self::schedule_election_timeout(&myself, state);
        }

        Ok(())
    }

    /// Handle a call command and return response
    pub fn handle_call_command(&self, state: &RaftState, json: serde_json::Value) -> CallResponse {
        let command = json
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        match command {
            "is_leader" => CallResponse::Success(serde_json::json!({
                "is_leader": state.is_leader(),
                "node_name": state.config.node_name,
                "term": state.current_term,
            })),
            "get_leader" => CallResponse::Success(serde_json::json!({
                "leader": state.current_leader,
                "term": state.current_term,
            })),
            "status" => CallResponse::Success(serde_json::json!({
                "node_name": state.config.node_name,
                "role": state.role.to_string(),
                "term": state.current_term,
                "leader": state.current_leader,
                "peers": state.peer_count(),
                "voted_for": state.voted_for,
            })),
            "peers" => {
                let peer_names: Vec<&String> = state.peers.keys().collect();
                CallResponse::Success(serde_json::json!({
                    "peers": peer_names,
                    "count": peer_names.len(),
                }))
            }
            _ => CallResponse::Error(format!(
                "Unknown command: {}. Use: is_leader, get_leader, status, peers",
                command
            )),
        }
    }
}

#[cfg(test)]
mod raft_tests;

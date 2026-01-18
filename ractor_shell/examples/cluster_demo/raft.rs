//! Raft-based leader election for cluster nodes.
//!
//! This module implements a simplified Raft consensus algorithm focused on leader election.
//! It provides actors that participate in leader election using typed messages for
//! direct peer-to-peer communication with full network tracing support.
//!
//! This is example code demonstrating how to build distributed actors with ractor_shell
//! introspection support. It is not part of the ractor_shell library.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ractor::Actor;
//! use cluster_demo::raft::{RaftNode, RaftConfig};
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
//! Once running, you can query the Raft node via the shell using typed RPC:
//!
//! ```text
//! ractor@local > call raft_node GetStatus {}
//! {"node_name": "node1", "role": "Follower", "term": 1, "leader": "node2", "peers": 2}
//!
//! ractor@local > call raft_node IsLeader {}
//! false
//!
//! ractor@local > call raft_node GetLeader {}
//! "node2"
//! ```

use std::collections::HashMap;
use std::time::Duration;

use ractor::rpc::CallResult;
use ractor::{pg, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use ractor_cluster::RactorClusterMessage;
use serde::{Deserialize, Serialize};

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

// ==================== Typed Raft Messages ====================

/// Typed message for Raft peer-to-peer communication.
///
/// This message type derives `RactorClusterMessage` for binary serialization
/// over the cluster network, enabling full tracing support.
///
/// Note: Uses tuple-style fields as required by RactorClusterMessage derive.
#[derive(RactorClusterMessage, Debug)]
#[ractor_shell]
pub enum RaftMessage {
    // ==================== Internal Timers ====================
    // These are sent locally via send_after, not over the network
    /// Election timeout fired (internal) - generation to detect stale timers
    ElectionTimeout(u64),
    /// Heartbeat timeout fired (internal) - generation
    HeartbeatTimeout(u64),
    /// Peer discovery timeout (internal) - generation
    DiscoverPeers(u64),

    // ==================== Peer Protocol Messages ====================
    // These are sent between Raft nodes over the cluster network
    /// Request vote: (term, candidate_name)
    RequestVote(u64, String),
    /// Vote response: (term, vote_granted, voter_name)
    VoteResponse(u64, bool, String),
    /// Heartbeat from leader: (term, leader_name)
    Heartbeat(u64, String),

    // ==================== Shell RPC Commands ====================
    // These are called from the shell for introspection
    /// Get full status of the Raft node
    #[rpc]
    GetStatus(RpcReplyPort<RaftStatus>),
    /// Check if this node is the current leader
    #[rpc]
    IsLeader(RpcReplyPort<bool>),
    /// Get the current leader's name (if known)
    #[rpc]
    GetLeader(RpcReplyPort<Option<String>>),
    /// Get list of known peer names
    #[rpc]
    GetPeers(RpcReplyPort<Vec<String>>),
}

/// Status information returned by GetStatus RPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftStatus {
    /// Name of this node
    pub node_name: String,
    /// Current role (Follower, Candidate, Leader)
    pub role: String,
    /// Current term number
    pub term: u64,
    /// Current leader (if known)
    pub leader: Option<String>,
    /// Number of known peers
    pub peers: usize,
    /// Who this node voted for in the current term
    pub voted_for: Option<String>,
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

/// Information about a remote Raft peer
#[derive(Debug, Clone)]
struct PeerInfo {
    /// Direct reference to the peer's RaftNode actor
    raft_ref: ActorRef<RaftMessage>,
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
    pub(crate) election_timer_generation: u64,
    /// Generation counter for heartbeat timers
    pub(crate) heartbeat_timer_generation: u64,
    /// Generation counter for peer discovery timers
    pub(crate) discovery_timer_generation: u64,
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

    pub fn peer_count(&self) -> usize {
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
    pub(crate) fn next_election_generation(&mut self) -> u64 {
        self.election_timer_generation += 1;
        self.election_timer_generation
    }

    /// Increment heartbeat timer generation and return the new value
    pub(crate) fn next_heartbeat_generation(&mut self) -> u64 {
        self.heartbeat_timer_generation += 1;
        self.heartbeat_timer_generation
    }

    /// Increment discovery timer generation and return the new value
    pub(crate) fn next_discovery_generation(&mut self) -> u64 {
        self.discovery_timer_generation += 1;
        self.discovery_timer_generation
    }

    /// Build a RaftStatus for RPC responses
    pub fn get_status(&self) -> RaftStatus {
        RaftStatus {
            node_name: self.config.node_name.clone(),
            role: self.role.to_string(),
            term: self.current_term,
            leader: self.current_leader.clone(),
            peers: self.peer_count(),
            voted_for: self.voted_for.clone(),
        }
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
/// This actor uses typed `RaftMessage` for direct peer-to-peer communication,
/// enabling full network-level tracing via ractor_cluster.
pub struct RaftNode;

impl Actor for RaftNode {
    type Msg = RaftMessage;
    type State = RaftState;
    type Arguments = RaftConfig;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        config: RaftConfig,
    ) -> Result<Self::State, ActorProcessingErr> {
        // Join the Raft cluster process group for peer discovery
        pg::join(RAFT_CLUSTER_GROUP.to_string(), vec![myself.get_cell()]);

        // Register schema for shell introspection
        if let Some(name) = myself.get_name() {
            ractor_shell::schema_registry::register::<RaftMessage>(&name);
        }

        let mut state = RaftState::new(config);

        // Schedule initial peer discovery
        let gen = state.next_discovery_generation();
        myself.send_after(Duration::from_millis(500), move || {
            RaftMessage::DiscoverPeers(gen)
        });

        // Schedule initial election timeout
        let timeout = state.election_timeout();
        let gen = state.next_election_generation();
        myself.send_after(timeout, move || RaftMessage::ElectionTimeout(gen));

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
            // ==================== Internal Timers ====================
            RaftMessage::DiscoverPeers(generation)
                if generation == state.discovery_timer_generation =>
            {
                tracing::trace!(
                    name = %state.config.node_name,
                    "DiscoverPeers timer fired"
                );
                self.discover_peers(myself, state).await?;
            }
            RaftMessage::ElectionTimeout(generation)
                if generation == state.election_timer_generation =>
            {
                tracing::debug!(
                    name = %state.config.node_name,
                    term = state.current_term,
                    role = ?state.role,
                    "ElectionTimeout - starting election"
                );
                self.handle_election_timeout(myself, state).await?;
            }
            RaftMessage::HeartbeatTimeout(generation)
                if generation == state.heartbeat_timer_generation =>
            {
                tracing::trace!(
                    name = %state.config.node_name,
                    term = state.current_term,
                    "HeartbeatTimeout - sending heartbeats"
                );
                self.handle_heartbeat_timeout(myself, state).await?;
            }

            // ==================== Peer Protocol ====================
            RaftMessage::RequestVote(term, ref candidate_name) => {
                tracing::info!(
                    name = %state.config.node_name,
                    from = %candidate_name,
                    term = term,
                    current_term = state.current_term,
                    "Received RequestVote"
                );
                self.handle_request_vote(myself, state, term, candidate_name.clone())
                    .await?;
            }
            RaftMessage::VoteResponse(term, vote_granted, ref voter_name) => {
                tracing::info!(
                    name = %state.config.node_name,
                    from = %voter_name,
                    term = term,
                    vote_granted = vote_granted,
                    "Received VoteResponse"
                );
                self.handle_vote_response(myself, state, term, vote_granted, voter_name.clone())
                    .await?;
            }
            RaftMessage::Heartbeat(term, ref leader_name) => {
                tracing::debug!(
                    name = %state.config.node_name,
                    from = %leader_name,
                    term = term,
                    "Received Heartbeat"
                );
                self.handle_heartbeat(myself, state, term, leader_name.clone())
                    .await?;
            }

            // ==================== Shell RPCs ====================
            RaftMessage::GetStatus(reply) => {
                tracing::debug!(
                    name = %state.config.node_name,
                    "RPC: GetStatus"
                );
                let _ = reply.send(state.get_status());
            }
            RaftMessage::IsLeader(reply) => {
                tracing::debug!(
                    name = %state.config.node_name,
                    is_leader = state.is_leader(),
                    "RPC: IsLeader"
                );
                let _ = reply.send(state.is_leader());
            }
            RaftMessage::GetLeader(reply) => {
                tracing::debug!(
                    name = %state.config.node_name,
                    leader = ?state.current_leader,
                    "RPC: GetLeader"
                );
                let _ = reply.send(state.current_leader.clone());
            }
            RaftMessage::GetPeers(reply) => {
                tracing::debug!(
                    name = %state.config.node_name,
                    peer_count = state.peers.len(),
                    "RPC: GetPeers"
                );
                let _ = reply.send(state.peers.keys().cloned().collect());
            }

            // Ignore stale timer events (generation mismatch)
            _ => {}
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        myself: ActorRef<Self::Msg>,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        // Unregister schema
        if let Some(name) = myself.get_name() {
            ractor_shell::schema_registry::unregister(&name);
        }
        Ok(())
    }
}

impl RaftNode {
    /// Discover peers by finding other RaftNodes in the cluster process group
    async fn discover_peers(
        &self,
        myself: ActorRef<RaftMessage>,
        state: &mut RaftState,
    ) -> Result<(), ActorProcessingErr> {
        // Find all Raft nodes in the cluster group
        let members = pg::get_members(&RAFT_CLUSTER_GROUP.to_string());

        // Filter for remote Raft nodes (skip local/self)
        for cell in members {
            if cell.get_id().is_local() {
                continue; // Skip self
            }

            let actor_id = cell.get_id().to_string();
            if state
                .peers
                .values()
                .any(|p| p.raft_ref.get_id().to_string() == actor_id)
            {
                continue; // Already known
            }

            // Convert to typed reference
            let raft_ref: ActorRef<RaftMessage> = ActorRef::from(cell.clone());

            // Call GetStatus RPC to get the peer's node name
            match raft_ref
                .call(RaftMessage::GetStatus, Some(Duration::from_millis(500)))
                .await
            {
                Ok(CallResult::Success(status)) => {
                    if status.node_name != state.config.node_name
                        && !state.peers.contains_key(&status.node_name)
                    {
                        tracing::info!(
                            local = %state.config.node_name,
                            peer = %status.node_name,
                            "Discovered Raft peer"
                        );

                        state
                            .peers
                            .insert(status.node_name.clone(), PeerInfo { raft_ref });
                    }
                }
                _ => {
                    // Failed to reach peer, skip
                }
            }
        }

        // Schedule next peer discovery
        let gen = state.next_discovery_generation();
        myself.send_after(
            Duration::from_millis(PEER_DISCOVERY_INTERVAL_MS),
            move || RaftMessage::DiscoverPeers(gen),
        );

        Ok(())
    }

    /// Broadcast a RequestVote to all peers
    fn broadcast_request_vote(&self, state: &RaftState, term: u64, candidate_name: &str) {
        let peer_names: Vec<_> = state.peers.keys().cloned().collect();
        tracing::info!(
            name = %candidate_name,
            term = term,
            peers = ?peer_names,
            "Broadcasting RequestVote"
        );
        for peer in state.peers.values() {
            let _ = peer
                .raft_ref
                .cast(RaftMessage::RequestVote(term, candidate_name.to_string()));
        }
    }

    /// Broadcast a Heartbeat to all peers
    fn broadcast_heartbeat(&self, state: &RaftState, term: u64, leader_name: &str) {
        tracing::trace!(
            name = %leader_name,
            term = term,
            peer_count = state.peers.len(),
            "Broadcasting Heartbeat"
        );
        for peer in state.peers.values() {
            let _ = peer
                .raft_ref
                .cast(RaftMessage::Heartbeat(term, leader_name.to_string()));
        }
    }

    /// Send a VoteResponse to a specific peer
    fn send_vote_response(
        &self,
        state: &RaftState,
        peer_name: &str,
        term: u64,
        vote_granted: bool,
        voter_name: &str,
    ) {
        tracing::info!(
            from = %voter_name,
            to = %peer_name,
            term = term,
            vote_granted = vote_granted,
            "Sending VoteResponse"
        );
        if let Some(peer) = state.peers.get(peer_name) {
            let _ = peer.raft_ref.cast(RaftMessage::VoteResponse(
                term,
                vote_granted,
                voter_name.to_string(),
            ));
        }
    }

    /// Schedule a new election timeout (invalidates any pending election timers)
    fn schedule_election_timeout(myself: &ActorRef<RaftMessage>, state: &mut RaftState) {
        let timeout = state.election_timeout();
        let gen = state.next_election_generation();
        myself.send_after(timeout, move || RaftMessage::ElectionTimeout(gen));
    }

    /// Schedule a new heartbeat timeout (invalidates any pending heartbeat timers)
    fn schedule_heartbeat_timeout(myself: &ActorRef<RaftMessage>, state: &mut RaftState) {
        let interval = state.heartbeat_interval();
        let gen = state.next_heartbeat_generation();
        myself.send_after(interval, move || RaftMessage::HeartbeatTimeout(gen));
    }

    /// Handle election timeout - transition to candidate and request votes
    async fn handle_election_timeout(
        &self,
        myself: ActorRef<RaftMessage>,
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
        self.broadcast_request_vote(state, state.current_term, &state.config.node_name.clone());

        // Check if we already have majority (single node cluster)
        self.check_election_result(&myself, state).await?;

        // Schedule next election timeout (with new generation)
        Self::schedule_election_timeout(&myself, state);

        Ok(())
    }

    /// Handle vote request from a candidate
    async fn handle_request_vote(
        &self,
        myself: ActorRef<RaftMessage>,
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
        self.send_vote_response(
            state,
            &candidate_name,
            state.current_term,
            vote_granted,
            &state.config.node_name.clone(),
        );

        Ok(())
    }

    /// Handle vote response
    async fn handle_vote_response(
        &self,
        myself: ActorRef<RaftMessage>,
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
        myself: &ActorRef<RaftMessage>,
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
            self.send_heartbeats(state);

            // Schedule heartbeat timer (with new generation)
            Self::schedule_heartbeat_timeout(myself, state);
        }

        Ok(())
    }

    /// Handle heartbeat timeout - send heartbeats to all peers
    async fn handle_heartbeat_timeout(
        &self,
        myself: ActorRef<RaftMessage>,
        state: &mut RaftState,
    ) -> Result<(), ActorProcessingErr> {
        if state.role != RaftRole::Leader {
            return Ok(());
        }

        self.send_heartbeats(state);

        // Schedule next heartbeat (with new generation)
        Self::schedule_heartbeat_timeout(&myself, state);

        Ok(())
    }

    /// Send heartbeats to all peers
    fn send_heartbeats(&self, state: &RaftState) {
        self.broadcast_heartbeat(state, state.current_term, &state.config.node_name.clone());
    }

    /// Handle heartbeat from leader
    async fn handle_heartbeat(
        &self,
        myself: ActorRef<RaftMessage>,
        state: &mut RaftState,
        term: u64,
        leader_name: String,
    ) -> Result<(), ActorProcessingErr> {
        // Log all heartbeat receptions at TRACE level for debugging
        tracing::trace!(
            node = %state.config.node_name,
            leader = %leader_name,
            term = term,
            "Received heartbeat"
        );

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
}

#[cfg(test)]
mod raft_tests;

//! Supervisor for RaftNode actors.
//!
//! This module provides a supervision actor that spawns and monitors RaftNode actors,
//! automatically restarting them on failure while preserving their configuration.
//!
//! ## Architecture
//!
//! ```text
//! RaftSupervisor (one per node)
//!     └─> RaftNode (supervised child)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ractor::Actor;
//! use cluster_demo::raft::{RaftConfig};
//! use cluster_demo::raft_supervisor::{RaftSupervisor, RaftSupervisorArgs};
//!
//! let supervisor_args = RaftSupervisorArgs {
//!     config: RaftConfig {
//!         node_name: "node1".to_string(),
//!         ..Default::default()
//!     },
//!     max_restarts: 5,
//! };
//!
//! let (supervisor_ref, _) = Actor::spawn(
//!     Some("raft_supervisor".to_string()),
//!     RaftSupervisor,
//!     supervisor_args
//! ).await?;
//! ```

use std::time::{Duration, Instant};

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};

use super::raft::{RaftConfig, RaftMessage, RaftNode};

/// Default maximum number of restarts within the restart window
const DEFAULT_MAX_RESTARTS: u32 = 5;

/// Time window for tracking restarts (60 seconds)
const RESTART_WINDOW: Duration = Duration::from_secs(60);

// ==================== Supervisor Actor ====================

/// Supervisor actor for RaftNode.
///
/// Spawns a RaftNode as a linked child and automatically restarts it on failure
/// while enforcing restart limits to prevent infinite restart loops.
pub struct RaftSupervisor;

/// Arguments for spawning a RaftSupervisor
#[derive(Debug, Clone)]
pub struct RaftSupervisorArgs {
    /// Configuration for the RaftNode (preserved across restarts)
    pub config: RaftConfig,
    /// Maximum number of restarts within the restart window (default: 5)
    pub max_restarts: u32,
}

impl Default for RaftSupervisorArgs {
    fn default() -> Self {
        Self {
            config: RaftConfig::default(),
            max_restarts: DEFAULT_MAX_RESTARTS,
        }
    }
}

/// State for the RaftSupervisor
pub struct RaftSupervisorState {
    /// RaftNode configuration (preserved across restarts)
    config: RaftConfig,
    /// Reference to the supervised RaftNode child
    raft_ref: Option<ActorRef<RaftMessage>>,
    /// Number of restarts that have occurred
    restart_count: u32,
    /// Maximum allowed restarts within the restart window
    max_restarts: u32,
    /// Time of the first restart in the current window
    first_restart_time: Option<Instant>,
}

impl Actor for RaftSupervisor {
    type Msg = ();
    type State = RaftSupervisorState;
    type Arguments = RaftSupervisorArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        tracing::info!(
            supervisor = %args.config.node_name,
            max_restarts = args.max_restarts,
            "Starting RaftSupervisor"
        );

        let mut state = RaftSupervisorState {
            config: args.config,
            raft_ref: None,
            restart_count: 0,
            max_restarts: args.max_restarts,
            first_restart_time: None,
        };

        // Spawn the RaftNode as a linked child
        state.spawn_raft_node(myself).await?;

        Ok(state)
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SupervisionEvent::ActorFailed(dead_actor, err) => {
                tracing::warn!(
                    supervisor = %state.config.node_name,
                    child_id = %dead_actor.get_id(),
                    error = %err,
                    restart_count = state.restart_count + 1,
                    max_restarts = state.max_restarts,
                    "RaftNode failed - attempting restart"
                );

                // Check if we should reset the restart counter
                let now = Instant::now();
                if let Some(first_restart) = state.first_restart_time {
                    if now.duration_since(first_restart) > RESTART_WINDOW {
                        // Outside the restart window - reset counter
                        tracing::debug!(
                            supervisor = %state.config.node_name,
                            "Restart window expired - resetting restart counter"
                        );
                        state.restart_count = 0;
                        state.first_restart_time = None;
                    }
                }

                // Check restart limits
                if state.restart_count >= state.max_restarts {
                    tracing::error!(
                        supervisor = %state.config.node_name,
                        restart_count = state.restart_count,
                        max_restarts = state.max_restarts,
                        window_secs = RESTART_WINDOW.as_secs(),
                        "RaftNode exceeded maximum restart limit - stopping supervisor"
                    );
                    return Err(From::from(format!(
                        "RaftNode exceeded {} restarts in {} seconds",
                        state.max_restarts,
                        RESTART_WINDOW.as_secs()
                    )));
                }

                // Increment restart counter
                state.restart_count += 1;
                if state.first_restart_time.is_none() {
                    state.first_restart_time = Some(now);
                }

                // Restart the RaftNode
                state.spawn_raft_node(myself).await?;

                tracing::info!(
                    supervisor = %state.config.node_name,
                    restart_count = state.restart_count,
                    "RaftNode restarted successfully"
                );
            }
            SupervisionEvent::ActorTerminated(dead_actor, _, reason) => {
                tracing::info!(
                    supervisor = %state.config.node_name,
                    child_id = %dead_actor.get_id(),
                    reason = ?reason,
                    "RaftNode terminated gracefully - no restart"
                );
                state.raft_ref = None;
            }
            SupervisionEvent::ActorStarted(_) => {
                // Ignore actor started events
            }
            SupervisionEvent::ProcessGroupChanged(_) => {
                // Ignore process group changes
            }
            SupervisionEvent::PidLifecycleEvent(_) => {
                // Ignore PID lifecycle events
            }
        }

        Ok(())
    }
}

impl RaftSupervisorState {
    /// Spawns a new RaftNode as a linked child.
    ///
    /// The RaftNode is always named "raft_node" to preserve registry visibility
    /// and will automatically rejoin the raft_cluster process group in its pre_start.
    async fn spawn_raft_node(
        &mut self,
        supervisor: ActorRef<()>,
    ) -> Result<(), ActorProcessingErr> {
        tracing::debug!(
            supervisor = %self.config.node_name,
            "Spawning RaftNode child"
        );

        let (raft_ref, _handle) = Actor::spawn_linked(
            Some("raft_node".to_string()),
            RaftNode,
            self.config.clone(),
            supervisor.get_cell(),
        )
        .await?;

        self.raft_ref = Some(raft_ref);
        Ok(())
    }
}

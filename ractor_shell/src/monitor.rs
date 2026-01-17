//! Actor monitoring and event tracking
//!
//! Provides functionality to monitor actor lifecycle events similar to Erlang's process monitoring.

use chrono::Local;
use colored::Colorize;
use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};
use std::collections::HashMap;

/// Events that can be monitored
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// Actor started
    ActorStarted {
        actor_id: String,
        actor_name: Option<String>,
        timestamp: chrono::DateTime<Local>,
    },
    /// Actor stopped normally
    ActorStopped {
        actor_id: String,
        actor_name: Option<String>,
        reason: String,
        timestamp: chrono::DateTime<Local>,
    },
    /// Actor panicked
    ActorPanicked {
        actor_id: String,
        actor_name: Option<String>,
        error: String,
        timestamp: chrono::DateTime<Local>,
    },
    /// Actor killed
    ActorKilled {
        actor_id: String,
        actor_name: Option<String>,
        timestamp: chrono::DateTime<Local>,
    },
}

impl MonitorEvent {
    /// Format event for display
    pub fn format(&self) -> String {
        match self {
            MonitorEvent::ActorStarted {
                actor_id,
                actor_name,
                timestamp,
            } => {
                let time = timestamp.format("%H:%M:%S%.3f");
                let name = actor_name.as_deref().unwrap_or("unnamed");
                format!(
                    "{} {} {} {} {}",
                    format!("[{}]", time).bright_black(),
                    "▲".green(),
                    "STARTED".green().bold(),
                    name.cyan(),
                    format!("({})", actor_id).bright_black()
                )
            }
            MonitorEvent::ActorStopped {
                actor_id,
                actor_name,
                reason,
                timestamp,
            } => {
                let time = timestamp.format("%H:%M:%S%.3f");
                let name = actor_name.as_deref().unwrap_or("unnamed");
                format!(
                    "{} {} {} {} {} - {}",
                    format!("[{}]", time).bright_black(),
                    "▼".yellow(),
                    "STOPPED".yellow().bold(),
                    name.cyan(),
                    format!("({})", actor_id).bright_black(),
                    reason.bright_black()
                )
            }
            MonitorEvent::ActorPanicked {
                actor_id,
                actor_name,
                error,
                timestamp,
            } => {
                let time = timestamp.format("%H:%M:%S%.3f");
                let name = actor_name.as_deref().unwrap_or("unnamed");
                format!(
                    "{} {} {} {} {} - {}",
                    format!("[{}]", time).bright_black(),
                    "✗".red(),
                    "PANICKED".red().bold(),
                    name.cyan(),
                    format!("({})", actor_id).bright_black(),
                    error.red()
                )
            }
            MonitorEvent::ActorKilled {
                actor_id,
                actor_name,
                timestamp,
            } => {
                let time = timestamp.format("%H:%M:%S%.3f");
                let name = actor_name.as_deref().unwrap_or("unnamed");
                format!(
                    "{} {} {} {} {}",
                    format!("[{}]", time).bright_black(),
                    "⊗".red(),
                    "KILLED".red().bold(),
                    name.cyan(),
                    format!("({})", actor_id).bright_black()
                )
            }
        }
    }
}

/// Message types for the MonitorActor
#[derive(Debug)]
pub enum MonitorMessage {
    /// Start monitoring an actor
    Monitor { actor_name: String },
    /// Stop monitoring an actor
    Unmonitor { actor_name: String },
    /// Report an event
    Event(MonitorEvent),
    /// Get list of monitored actors
    GetMonitored(ractor::RpcReplyPort<Vec<String>>),
    /// Clear all monitors
    ClearAll,
}

impl ractor::Message for MonitorMessage {}

/// State for the monitor actor
pub struct MonitorState {
    /// Currently monitored actor names
    monitored: HashMap<String, ractor::ActorId>,
    /// Event history (last N events)
    event_history: Vec<MonitorEvent>,
    /// Maximum events to keep in history
    max_history: usize,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            monitored: HashMap::new(),
            event_history: Vec::new(),
            max_history: 100,
        }
    }
}

/// Actor that monitors other actors and displays lifecycle events
pub struct MonitorActor;

impl Actor for MonitorActor {
    type Msg = MonitorMessage;
    type State = MonitorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        println!("{} Monitor system started", "✓".green());
        Ok(MonitorState::default())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            MonitorMessage::Monitor { actor_name } => {
                // Try to find the actor in registry
                if let Some(cell) = ractor::registry::where_is(actor_name.clone()) {
                    let actor_id = cell.get_id();
                    state.monitored.insert(actor_name.clone(), actor_id);

                    println!(
                        "{} Monitoring {} {}",
                        "✓".green(),
                        actor_name.cyan(),
                        format!("({})", actor_id).bright_black()
                    );

                    // Record start event if actor is currently running
                    let event = MonitorEvent::ActorStarted {
                        actor_id: actor_id.to_string(),
                        actor_name: Some(actor_name),
                        timestamp: Local::now(),
                    };
                    state.event_history.push(event);
                    if state.event_history.len() > state.max_history {
                        state.event_history.remove(0);
                    }
                } else {
                    println!("{} Actor '{}' not found in registry", "✗".red(), actor_name);
                }
            }

            MonitorMessage::Unmonitor { actor_name } => {
                if state.monitored.remove(&actor_name).is_some() {
                    println!("{} Stopped monitoring {}", "✓".green(), actor_name.cyan());
                } else {
                    println!("{} Not monitoring '{}'", "✗".yellow(), actor_name);
                }
            }

            MonitorMessage::Event(event) => {
                // Display the event
                println!("{}", event.format());

                // Add to history
                state.event_history.push(event);
                if state.event_history.len() > state.max_history {
                    state.event_history.remove(0);
                }
            }

            MonitorMessage::GetMonitored(reply) => {
                let monitored: Vec<String> = state.monitored.keys().cloned().collect();
                let _ = reply.send(monitored);
            }

            MonitorMessage::ClearAll => {
                let count = state.monitored.len();
                state.monitored.clear();
                println!("{} Cleared {} monitors", "✓".green(), count);
            }
        }

        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        // Convert supervision events to monitor events
        match message {
            SupervisionEvent::ActorStarted(actor_cell) => {
                let actor_id = actor_cell.get_id();
                let actor_name = actor_cell.get_name();

                // Only report if we're monitoring this actor
                if let Some(name) = &actor_name {
                    if state.monitored.contains_key(name) {
                        let event = MonitorEvent::ActorStarted {
                            actor_id: actor_id.to_string(),
                            actor_name,
                            timestamp: Local::now(),
                        };
                        myself.cast(MonitorMessage::Event(event))?;
                    }
                }
            }

            SupervisionEvent::ActorTerminated(actor_cell, _, reason) => {
                let actor_id = actor_cell.get_id();
                let actor_name = actor_cell.get_name();

                // Check if we're monitoring this actor
                if let Some(name) = &actor_name {
                    if state.monitored.contains_key(name) {
                        let event = MonitorEvent::ActorStopped {
                            actor_id: actor_id.to_string(),
                            actor_name,
                            reason: format!("{:?}", reason),
                            timestamp: Local::now(),
                        };
                        myself.cast(MonitorMessage::Event(event))?;
                    }
                }
            }

            SupervisionEvent::ActorFailed(actor_cell, error) => {
                let actor_id = actor_cell.get_id();
                let actor_name = actor_cell.get_name();

                if let Some(name) = &actor_name {
                    if state.monitored.contains_key(name) {
                        let event = MonitorEvent::ActorPanicked {
                            actor_id: actor_id.to_string(),
                            actor_name,
                            error: error.to_string(),
                            timestamp: Local::now(),
                        };
                        myself.cast(MonitorMessage::Event(event))?;
                    }
                }
            }

            _ => {} // Ignore other events
        }

        Ok(())
    }
}

/// Helper to check if monitoring is available
pub fn is_monitoring_supported() -> bool {
    // Monitoring is always supported in ractor
    true
}

#[cfg(test)]
mod monitor_tests;

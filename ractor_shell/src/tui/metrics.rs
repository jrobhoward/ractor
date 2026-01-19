//! Actor metrics collection using tracing spans and ractor APIs.
//!
//! This module provides metrics collection for actors without requiring
//! changes to ractor core. It uses:
//!
//! 1. `ractor::registry` for actor enumeration
//! 2. `ractor::pg` for process group membership
//! 3. Tracing spans to approximate message processing counts

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use ractor::ActorStatus;
use ractor::{pg, registry};

/// Metrics for a single actor.
#[derive(Debug, Clone)]
pub struct ActorMetrics {
    /// Actor ID as string (e.g., "0.1")
    pub id: String,
    /// Actor name (if registered)
    pub name: Option<String>,
    /// Current status
    pub status: ActorStatus,
    /// Process groups this actor belongs to
    pub groups: Vec<String>,
    /// When we first observed this actor
    pub first_seen: Instant,
    /// Cached uptime in seconds (snapshot at refresh time for stable display)
    pub uptime_secs: f64,
    /// Approximate message count (from tracing spans)
    pub message_count: u64,
    /// Cumulative time spent in message handlers (nanoseconds)
    pub handle_time_ns: u64,
}

impl ActorMetrics {
    /// Calculate uptime since first observation.
    pub fn uptime(&self) -> std::time::Duration {
        self.first_seen.elapsed()
    }

    /// Format uptime as human-readable string.
    pub fn uptime_string(&self) -> String {
        let duration = self.uptime();
        let secs = duration.as_secs();

        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Calculate message rate (messages per second).
    /// Uses cached uptime_secs for stable display between refreshes.
    pub fn msg_rate(&self) -> f64 {
        if self.uptime_secs > 0.0 {
            self.message_count as f64 / self.uptime_secs
        } else {
            0.0
        }
    }

    /// Format message rate as human-readable string.
    pub fn msg_rate_string(&self) -> String {
        let rate = self.msg_rate();
        if rate >= 1000.0 {
            format!("{:.1}k/s", rate / 1000.0)
        } else if rate >= 1.0 {
            format!("{:.1}/s", rate)
        } else {
            "0/s".to_string()
        }
    }
}

/// Shared metrics storage for actor data.
///
/// This collector gathers metrics from multiple sources:
/// - Registry queries for actor enumeration
/// - Process group membership
/// - Tracing spans for message counts (when available)
#[derive(Debug, Clone)]
pub struct ActorMetricsCollector {
    /// Per-actor metrics, keyed by actor ID string
    actors: Arc<DashMap<String, ActorMetricsEntry>>,
    /// Message counts from tracing (actor name -> count)
    message_counts: Arc<DashMap<String, AtomicU64>>,
}

#[derive(Debug)]
struct ActorMetricsEntry {
    name: Option<String>,
    status: ActorStatus,
    groups: Vec<String>,
    first_seen: Instant,
}

impl Default for ActorMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorMetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            actors: Arc::new(DashMap::new()),
            message_counts: Arc::new(DashMap::new()),
        }
    }

    /// Refresh metrics from ractor registry and process groups.
    ///
    /// This queries the current state and updates our internal tracking.
    pub fn refresh(&self) {
        // Get all registered actor names
        let registered_names = registry::registered();

        // Build a map of actor ID -> groups
        let mut actor_groups: HashMap<String, Vec<String>> = HashMap::new();

        // Query known process groups
        // Note: ractor doesn't expose a way to list all groups, so we check common ones
        for group_name in crate::KNOWN_PROCESS_GROUPS.iter() {
            // Get members from the default scope
            let members = pg::get_members(&group_name.to_string());
            for member in members {
                let id_str = member.get_id().to_string();
                let groups = actor_groups.entry(id_str).or_default();
                if !groups.contains(&group_name.to_string()) {
                    groups.push(group_name.to_string());
                }
            }
        }

        // Update metrics for registered actors
        for name in registered_names {
            // Look up the actor cell by name
            if let Some(actor_cell) = registry::where_is(name.clone()) {
                let id_str = actor_cell.get_id().to_string();
                let status = actor_cell.get_status();
                let groups = actor_groups.remove(&id_str).unwrap_or_default();

                self.actors
                    .entry(id_str.clone())
                    .and_modify(|entry| {
                        entry.name = Some(name.clone());
                        entry.status = status;
                        entry.groups = groups.clone();
                    })
                    .or_insert_with(|| ActorMetricsEntry {
                        name: Some(name),
                        status,
                        groups,
                        first_seen: Instant::now(),
                    });
            }
        }

        // Mark actors that are no longer in registry as stopped
        // (but keep them for a while so user can see they stopped)
        let registered_ids: std::collections::HashSet<_> = self
            .actors
            .iter()
            .filter(|e| e.value().name.is_some())
            .map(|e| e.key().clone())
            .collect();

        for mut entry in self.actors.iter_mut() {
            if entry.value().name.is_some() && !registered_ids.contains(entry.key()) {
                entry.value_mut().status = ActorStatus::Stopped;
            }
        }
    }

    /// Get all current actor metrics.
    pub fn get_all(&self) -> Vec<ActorMetrics> {
        self.actors
            .iter()
            .map(|entry| {
                let id = entry.key().clone();

                // Try to get metrics from ActorCell if available
                let (message_count, handle_time_ns) = if let Some(name) = &entry.value().name {
                    registry::where_is(name.clone())
                        .map(|cell| (cell.get_message_count(), cell.get_handle_time_ns()))
                        .unwrap_or((0, 0))
                } else {
                    (0, 0)
                };

                ActorMetrics {
                    id,
                    name: entry.value().name.clone(),
                    status: entry.value().status,
                    groups: entry.value().groups.clone(),
                    first_seen: entry.value().first_seen,
                    uptime_secs: entry.value().first_seen.elapsed().as_secs_f64(),
                    message_count,
                    handle_time_ns,
                }
            })
            .collect()
    }

    /// Increment message count for an actor (called from tracing layer).
    pub fn increment_message_count(&self, actor_name: &str) {
        self.message_counts
            .entry(actor_name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Get summary statistics.
    pub fn summary(&self) -> MetricsSummary {
        let mut total = 0;
        let mut running = 0;
        let mut stopped = 0;
        let mut other = 0;

        for entry in self.actors.iter() {
            total += 1;
            match entry.value().status {
                ActorStatus::Running => running += 1,
                ActorStatus::Stopped => stopped += 1,
                _ => other += 1,
            }
        }

        MetricsSummary {
            total,
            running,
            stopped,
            other,
        }
    }

    /// Remove actors that have been stopped for too long.
    pub fn prune_stopped(&self, max_age: std::time::Duration) {
        self.actors.retain(|_, entry| {
            if entry.status == ActorStatus::Stopped {
                entry.first_seen.elapsed() < max_age
            } else {
                true
            }
        });
    }
}

/// Summary statistics for all actors.
#[derive(Debug, Clone, Default)]
pub struct MetricsSummary {
    pub total: usize,
    pub running: usize,
    pub stopped: usize,
    pub other: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uptime_formatting() {
        let metrics = ActorMetrics {
            id: "0.1".to_string(),
            name: Some("test".to_string()),
            status: ActorStatus::Running,
            groups: vec![],
            first_seen: Instant::now() - std::time::Duration::from_secs(125),
            uptime_secs: 125.0,
            message_count: 0,
            handle_time_ns: 0,
        };

        let uptime = metrics.uptime_string();
        assert!(uptime.contains("2m"), "Expected '2m' in uptime: {}", uptime);
    }

    #[test]
    fn test_metrics_collector_new() {
        let collector = ActorMetricsCollector::new();
        assert!(collector.get_all().is_empty());
    }

    #[test]
    fn test_summary_default() {
        let summary = MetricsSummary::default();
        assert_eq!(summary.total, 0);
        assert_eq!(summary.running, 0);
    }
}

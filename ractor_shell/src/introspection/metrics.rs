//! Metrics Collection
//!
//! This module contains functionality for collecting actor and system metrics.

use std::collections::HashSet;
use std::time::Instant;

use ractor::{pg, registry};

use crate::protocol::{RemoteActorMetrics, SystemInfo};

/// Collect metrics for all actors on this node (for remote `top` command)
pub(crate) fn collect_actor_metrics() -> Vec<RemoteActorMetrics> {
    // We'll use a simple approach: collect all actors from registry and process groups,
    // and return basic metrics. Since we don't have access to the TUI's ActorMetricsCollector
    // state, we'll compute metrics fresh each time.

    let mut all_actors = Vec::new();
    let mut seen_ids = HashSet::new();

    // Track first-seen times (in a real implementation, this would be persisted)
    // For now, we'll just use "now" as the first-seen time, giving uptime of 0
    // This is a limitation - we'd need state to track actual first-seen times
    static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start_time = START_TIME.get_or_init(Instant::now);

    // Build a map of actor ID -> groups
    let mut actor_groups: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    // Query known process groups
    for group_name in crate::KNOWN_PROCESS_GROUPS.iter() {
        let members = pg::get_members(&group_name.to_string());
        for member in &members {
            let id_str = member.get_id().to_string();
            let groups = actor_groups.entry(id_str).or_default();
            if !groups.contains(&group_name.to_string()) {
                groups.push(group_name.to_string());
            }
        }
    }

    // Collect all actors from registry
    for name in registry::registered() {
        if let Some(cell) = registry::where_is(name.clone()) {
            let id = cell.get_id();
            if !id.is_local() {
                continue; // Skip remote actors
            }
            if seen_ids.insert(id) {
                let id_str = id.to_string();
                let groups = actor_groups.remove(&id_str).unwrap_or_default();

                all_actors.push(RemoteActorMetrics {
                    id: id_str,
                    name: Some(name),
                    status: format!("{:?}", cell.get_status()),
                    groups,
                    uptime_ms: start_time.elapsed().as_millis() as u64,
                    message_count: cell.get_message_count(),
                    handle_time_ns: cell.get_handle_time_ns(),
                });
            }
        }
    }

    // Also collect from process groups (for actors not in registry)
    for group_name in crate::KNOWN_PROCESS_GROUPS.iter() {
        for cell in pg::get_members(&group_name.to_string()) {
            let id = cell.get_id();
            if !id.is_local() {
                continue; // Skip remote actors
            }
            if seen_ids.insert(id) {
                let id_str = id.to_string();
                let groups = actor_groups.remove(&id_str).unwrap_or_default();

                all_actors.push(RemoteActorMetrics {
                    id: id_str,
                    name: cell.get_name(),
                    status: format!("{:?}", cell.get_status()),
                    groups,
                    uptime_ms: start_time.elapsed().as_millis() as u64,
                    message_count: cell.get_message_count(),
                    handle_time_ns: cell.get_handle_time_ns(),
                });
            }
        }
    }

    all_actors
}

/// Collector for system/process information that caches the sysinfo::System instance.
///
/// CPU usage calculation requires two measurements to compute a delta, so we must
/// keep the System instance alive between refreshes. Creating a new System each time
/// would always return 0.0% CPU.
#[cfg(feature = "sysinfo")]
pub struct SystemInfoCollector {
    sys: sysinfo::System,
    pid: sysinfo::Pid,
    hostname: String,
    exe_name: String,
    total_memory_bytes: u64,
}

#[cfg(feature = "sysinfo")]
impl SystemInfoCollector {
    /// Create a new collector. The first call to `refresh()` will return 0% CPU
    /// because there's no baseline yet. Subsequent calls will return accurate CPU%.
    pub fn new() -> Self {
        use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};

        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_processes(ProcessRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        let pid = sysinfo::Pid::from_u32(std::process::id());

        // Do initial refresh to establish baseline for CPU calculation
        // Note: Must use ProcessesToUpdate::All for CPU% to work correctly.
        // ProcessesToUpdate::Some doesn't properly track CPU timing data.
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );

        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        let exe_name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        let total_memory_bytes = sys.total_memory();

        Self {
            sys,
            pid,
            hostname,
            exe_name,
            total_memory_bytes,
        }
    }

    /// Refresh and return current system info.
    ///
    /// CPU% is calculated as delta since last refresh, so it reflects usage
    /// over the refresh interval (not instantaneous).
    pub fn refresh(&mut self) -> SystemInfo {
        use sysinfo::ProcessRefreshKind;

        // Must use ProcessesToUpdate::All for CPU% to work correctly.
        // ProcessesToUpdate::Some doesn't properly track CPU timing data in sysinfo 0.32.
        // This is more expensive but necessary for accurate CPU measurement.
        self.sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );

        let (cpu_percent, memory_bytes, process_uptime_secs, thread_count) =
            if let Some(process) = self.sys.process(self.pid) {
                (
                    process.cpu_usage(),
                    process.memory(),
                    process.run_time(),
                    0usize, // sysinfo doesn't expose thread count on all platforms
                )
            } else {
                (0.0, 0, 0, 0)
            };

        SystemInfo {
            hostname: self.hostname.clone(),
            exe_name: self.exe_name.clone(),
            pid: std::process::id(),
            cpu_percent,
            memory_bytes,
            process_uptime_secs,
            thread_count,
            total_memory_bytes: self.total_memory_bytes,
            ractor_shell_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(feature = "sysinfo")]
impl Default for SystemInfoCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Fallback SystemInfoCollector when sysinfo feature is disabled.
#[cfg(not(feature = "sysinfo"))]
pub struct SystemInfoCollector {
    hostname: String,
    exe_name: String,
}

#[cfg(not(feature = "sysinfo"))]
impl SystemInfoCollector {
    pub fn new() -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let exe_name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        Self { hostname, exe_name }
    }

    pub fn refresh(&mut self) -> SystemInfo {
        SystemInfo {
            hostname: self.hostname.clone(),
            exe_name: self.exe_name.clone(),
            pid: std::process::id(),
            cpu_percent: 0.0,
            memory_bytes: 0,
            process_uptime_secs: 0,
            thread_count: 0,
            total_memory_bytes: 0,
            ractor_shell_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(not(feature = "sysinfo"))]
impl Default for SystemInfoCollector {
    fn default() -> Self {
        Self::new()
    }
}

//! Application state and event loop for the TUI.

use super::metrics::{ActorMetrics, ActorMetricsCollector};
use super::ui;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ractor::rpc::CallResult;
use ractor::ActorRef;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

use crate::introspection::SystemInfoCollector;
use crate::protocol::{RemoteActorMetrics, ShellProtocolMessage, SystemInfo};
use crate::DEFAULT_RPC_TIMEOUT;

/// Sort column for the actor table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    Id,
    Status,
    Uptime,
    MsgRate,
    Groups,
}

impl SortColumn {
    /// Cycle to the next sort column.
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Id,
            Self::Id => Self::Status,
            Self::Status => Self::Uptime,
            Self::Uptime => Self::MsgRate,
            Self::MsgRate => Self::Groups,
            Self::Groups => Self::Name,
        }
    }

    /// Get display name for the column.
    pub fn name(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Id => "ID",
            Self::Status => "Status",
            Self::Uptime => "Uptime",
            Self::MsgRate => "Msg/s",
            Self::Groups => "Groups",
        }
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn toggle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    pub fn indicator(self) -> &'static str {
        match self {
            Self::Ascending => "▲",
            Self::Descending => "▼",
        }
    }
}

/// Main application state.
pub struct App {
    /// Should we exit the application?
    pub should_quit: bool,
    /// Metrics collector (for local mode)
    pub metrics: ActorMetricsCollector,
    /// Cached actor list (refreshed periodically)
    pub actors: Vec<ActorMetrics>,
    /// Currently selected row in the table
    pub selected: usize,
    /// Sort column
    pub sort_column: SortColumn,
    /// Sort direction
    pub sort_direction: SortDirection,
    /// Filter string (empty = no filter)
    pub filter: String,
    /// Is filter input mode active?
    pub filter_mode: bool,
    /// Refresh interval for actor metrics
    pub refresh_interval: Duration,
    /// Last refresh time for actor metrics
    pub last_refresh: Instant,
    /// Show help overlay?
    pub show_help: bool,
    /// Remote node connection (node_name, introspection_ref)
    pub remote_node: Option<(String, ActorRef<ShellProtocolMessage>)>,
    /// Last error message (for display)
    pub last_error: Option<String>,
    /// Cached system info (refreshed less frequently)
    pub system_info: Option<SystemInfo>,
    /// Refresh interval for system info (less frequent than actor metrics)
    pub sysinfo_refresh_interval: Duration,
    /// Last system info refresh time
    pub last_sysinfo_refresh: Instant,
    /// Cached system info collector for accurate CPU% (local mode only)
    sysinfo_collector: SystemInfoCollector,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new application instance for local monitoring.
    pub fn new() -> Self {
        Self {
            should_quit: false,
            metrics: ActorMetricsCollector::new(),
            actors: Vec::new(),
            selected: 0,
            sort_column: SortColumn::default(),
            sort_direction: SortDirection::default(),
            filter: String::new(),
            filter_mode: false,
            refresh_interval: Duration::from_secs(1),
            last_refresh: Instant::now() - Duration::from_secs(10), // Force immediate refresh
            show_help: false,
            remote_node: None,
            last_error: None,
            system_info: None,
            sysinfo_refresh_interval: Duration::from_secs(5),
            last_sysinfo_refresh: Instant::now() - Duration::from_secs(10), // Force immediate refresh
            sysinfo_collector: SystemInfoCollector::new(),
        }
    }

    /// Create a new application instance for remote monitoring.
    pub fn new_remote(
        node_name: String,
        introspection_ref: ActorRef<ShellProtocolMessage>,
    ) -> Self {
        Self {
            should_quit: false,
            metrics: ActorMetricsCollector::new(),
            actors: Vec::new(),
            selected: 0,
            sort_column: SortColumn::default(),
            sort_direction: SortDirection::default(),
            filter: String::new(),
            filter_mode: false,
            refresh_interval: Duration::from_secs(1),
            last_refresh: Instant::now() - Duration::from_secs(10), // Force immediate refresh
            show_help: false,
            remote_node: Some((node_name, introspection_ref)),
            last_error: None,
            system_info: None,
            sysinfo_refresh_interval: Duration::from_secs(5),
            last_sysinfo_refresh: Instant::now() - Duration::from_secs(10), // Force immediate refresh
            sysinfo_collector: SystemInfoCollector::new(),
        }
    }

    /// Run the TUI application for local monitoring.
    ///
    /// This takes over the terminal and runs until the user exits.
    pub async fn run() -> anyhow::Result<()> {
        let app = App::new();
        Self::run_app(app).await
    }

    /// Run the TUI application for remote monitoring.
    ///
    /// This takes over the terminal and runs until the user exits.
    pub async fn run_remote(
        node_name: String,
        introspection_ref: ActorRef<ShellProtocolMessage>,
    ) -> anyhow::Result<()> {
        let app = App::new_remote(node_name, introspection_ref);
        Self::run_app(app).await
    }

    /// Internal: run the TUI with the given app state.
    async fn run_app(mut app: App) -> anyhow::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run main loop
        let result = app.main_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Main event loop.
    async fn main_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> anyhow::Result<()> {
        loop {
            // Refresh metrics if needed
            if self.last_refresh.elapsed() >= self.refresh_interval {
                self.refresh_metrics().await;
            }

            // Refresh system info less frequently
            if self.last_sysinfo_refresh.elapsed() >= self.sysinfo_refresh_interval {
                self.refresh_system_info().await;
            }

            // Draw UI
            terminal.draw(|frame| ui::render(frame, self))?;

            // Handle events with timeout (to allow periodic refresh)
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key.code, key.modifiers);
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Refresh metrics from ractor (local or remote).
    async fn refresh_metrics(&mut self) {
        self.last_error = None;

        if let Some((ref node_name, ref introspection_ref)) = self.remote_node {
            // Remote mode: fetch metrics via RPC
            match introspection_ref
                .call(
                    ShellProtocolMessage::GetActorMetrics,
                    Some(DEFAULT_RPC_TIMEOUT),
                )
                .await
            {
                Ok(CallResult::Success(remote_metrics)) => {
                    // Convert RemoteActorMetrics to ActorMetrics
                    self.actors = remote_metrics
                        .into_iter()
                        .map(Self::convert_remote_metrics)
                        .collect();
                }
                Ok(CallResult::Timeout) => {
                    self.last_error = Some(format!("Timeout fetching metrics from {}", node_name));
                }
                Ok(CallResult::SenderError) => {
                    self.last_error = Some(format!("Connection lost to {}", node_name));
                }
                Err(e) => {
                    self.last_error = Some(format!("Error: {:?}", e));
                }
            }
        } else {
            // Local mode: use the metrics collector
            self.metrics.refresh();
            self.actors = self.metrics.get_all();
        }

        self.sort_actors();
        self.apply_filter();
        self.last_refresh = Instant::now();

        // Ensure selected index is valid
        if !self.actors.is_empty() && self.selected >= self.actors.len() {
            self.selected = self.actors.len() - 1;
        }
    }

    /// Refresh system info (local or remote).
    async fn refresh_system_info(&mut self) {
        if let Some((ref node_name, ref introspection_ref)) = self.remote_node {
            // Remote mode: fetch system info via RPC
            match introspection_ref
                .call(
                    ShellProtocolMessage::GetSystemInfo,
                    Some(DEFAULT_RPC_TIMEOUT),
                )
                .await
            {
                Ok(CallResult::Success(info)) => {
                    self.system_info = Some(info);
                }
                Ok(CallResult::Timeout) => {
                    // Don't overwrite last_error, keep actor metrics error if any
                    if self.last_error.is_none() {
                        self.last_error =
                            Some(format!("Timeout fetching system info from {}", node_name));
                    }
                }
                Ok(CallResult::SenderError) => {
                    if self.last_error.is_none() {
                        self.last_error = Some(format!("Connection lost to {}", node_name));
                    }
                }
                Err(_) => {
                    // Silently ignore - system info is optional
                }
            }
        } else {
            // Local mode: use cached collector for accurate CPU%
            self.system_info = Some(self.sysinfo_collector.refresh());
        }

        self.last_sysinfo_refresh = Instant::now();
    }

    /// Convert remote metrics to local ActorMetrics format.
    fn convert_remote_metrics(remote: RemoteActorMetrics) -> ActorMetrics {
        use ractor::ActorStatus;

        // Parse status string back to ActorStatus
        let status = match remote.status.as_str() {
            "Running" => ActorStatus::Running,
            "Stopped" => ActorStatus::Stopped,
            "Starting" => ActorStatus::Starting,
            "Stopping" => ActorStatus::Stopping,
            "Upgrading" => ActorStatus::Upgrading,
            "Draining" => ActorStatus::Draining,
            _ => ActorStatus::Running, // Default fallback
        };

        ActorMetrics {
            id: remote.id,
            name: remote.name,
            status,
            groups: remote.groups,
            // Convert uptime_ms back to first_seen (approximate)
            first_seen: Instant::now() - Duration::from_millis(remote.uptime_ms),
            uptime_secs: remote.uptime_ms as f64 / 1000.0,
            message_count: remote.message_count,
            handle_time_ns: remote.handle_time_ns,
        }
    }

    /// Sort actors according to current sort settings.
    fn sort_actors(&mut self) {
        let direction = self.sort_direction;

        self.actors.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::Name => a
                    .name
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.name.as_deref().unwrap_or("")),
                SortColumn::Id => a.id.cmp(&b.id),
                SortColumn::Status => format!("{:?}", a.status).cmp(&format!("{:?}", b.status)),
                SortColumn::Uptime => a.first_seen.cmp(&b.first_seen),
                SortColumn::MsgRate => a
                    .msg_rate()
                    .partial_cmp(&b.msg_rate())
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Groups => a.groups.len().cmp(&b.groups.len()),
            };

            match direction {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }

    /// Apply filter to actors list.
    fn apply_filter(&mut self) {
        if !self.filter.is_empty() {
            let filter_lower = self.filter.to_lowercase();
            self.actors.retain(|a| {
                a.name
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase().contains(&filter_lower))
                    || a.id.to_lowercase().contains(&filter_lower)
            });
        }
    }

    /// Handle keyboard input.
    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        // Handle filter mode separately
        if self.filter_mode {
            match code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                }
                KeyCode::Enter => {
                    self.filter_mode = false;
                    self.force_refresh(); // Trigger refresh on next loop iteration
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                }
                _ => {}
            }
            return;
        }

        // Handle help overlay
        if self.show_help {
            self.show_help = false;
            return;
        }

        match code {
            // Quit
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }

            // Navigation
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.actors.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !self.actors.is_empty() {
                    self.selected = self.actors.len() - 1;
                }
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.selected = (self.selected + 10).min(self.actors.len().saturating_sub(1));
            }

            // Sorting
            KeyCode::Char('s') => {
                self.sort_column = self.sort_column.next();
                self.sort_actors();
            }
            KeyCode::Char('S') => {
                self.sort_direction = self.sort_direction.toggle();
                self.sort_actors();
            }

            // Filter
            KeyCode::Char('/') => {
                self.filter_mode = true;
            }
            KeyCode::Char('c') => {
                // Clear filter
                self.filter.clear();
                self.force_refresh();
            }

            // Refresh
            KeyCode::Char('r') => {
                self.force_refresh();
            }

            // Help
            KeyCode::Char('?') => {
                self.show_help = true;
            }

            _ => {}
        }
    }

    /// Force a refresh on the next loop iteration.
    fn force_refresh(&mut self) {
        self.last_refresh = Instant::now() - Duration::from_secs(10);
    }

    /// Get the filtered actors list for display.
    pub fn visible_actors(&self) -> &[ActorMetrics] {
        &self.actors
    }

    /// Get the node name if connected to a remote node.
    pub fn node_name(&self) -> Option<&str> {
        self.remote_node.as_ref().map(|(name, _)| name.as_str())
    }

    /// Check if running in remote mode.
    pub fn is_remote(&self) -> bool {
        self.remote_node.is_some()
    }
}

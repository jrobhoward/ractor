//! Application state and event loop for the TUI.

use super::metrics::{ActorMetrics, ActorMetricsCollector};
use super::ui;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

/// Sort column for the actor table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    Id,
    Status,
    Uptime,
    Groups,
}

impl SortColumn {
    /// Cycle to the next sort column.
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Id,
            Self::Id => Self::Status,
            Self::Status => Self::Uptime,
            Self::Uptime => Self::Groups,
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
    /// Metrics collector
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
    /// Refresh interval
    pub refresh_interval: Duration,
    /// Last refresh time
    pub last_refresh: Instant,
    /// Show help overlay?
    pub show_help: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new application instance.
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
        }
    }

    /// Run the TUI application.
    ///
    /// This takes over the terminal and runs until the user exits.
    pub async fn run() -> anyhow::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create app and run main loop
        let mut app = App::new();
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
                self.refresh_metrics();
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

    /// Refresh metrics from ractor.
    fn refresh_metrics(&mut self) {
        self.metrics.refresh();
        self.actors = self.metrics.get_all();
        self.sort_actors();
        self.apply_filter();
        self.last_refresh = Instant::now();

        // Ensure selected index is valid
        if !self.actors.is_empty() && self.selected >= self.actors.len() {
            self.selected = self.actors.len() - 1;
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
                    self.refresh_metrics(); // Apply filter
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
                self.refresh_metrics();
            }

            // Refresh
            KeyCode::Char('r') => {
                self.refresh_metrics();
            }

            // Help
            KeyCode::Char('?') => {
                self.show_help = true;
            }

            _ => {}
        }
    }

    /// Get the filtered actors list for display.
    pub fn visible_actors(&self) -> &[ActorMetrics] {
        &self.actors
    }
}

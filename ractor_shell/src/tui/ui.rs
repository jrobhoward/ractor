//! UI rendering for the TUI dashboard.

use super::app::{App, SortColumn};
use ractor::ActorStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

/// Main render function.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Table
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_table(frame, app, chunks[1]);
    render_status_bar(frame, app, chunks[2]);

    // Render overlays
    if app.show_help {
        render_help_overlay(frame);
    }

    if app.filter_mode {
        render_filter_input(frame, app);
    }
}

/// Render the header bar.
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    // Build title based on local vs remote mode
    let title = if let Some(node_name) = app.node_name() {
        Line::from(vec![
            Span::styled("ractor top", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" - "),
            Span::styled(node_name, Style::default().fg(Color::Yellow).bold()),
            Span::styled(" (remote)", Style::default().fg(Color::Yellow)),
        ])
    } else {
        Line::from(vec![
            Span::styled("ractor top", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" - Actor Dashboard"),
        ])
    };

    // Count actors from the cached list (works for both local and remote)
    let total = app.actors.len();
    let running = app
        .actors
        .iter()
        .filter(|a| a.status == ActorStatus::Running)
        .count();
    let stopped = app
        .actors
        .iter()
        .filter(|a| a.status == ActorStatus::Stopped)
        .count();

    let stats = if let Some(ref error) = app.last_error {
        // Show error instead of stats
        Line::from(vec![
            Span::styled("  Error: ", Style::default().fg(Color::Red).bold()),
            Span::styled(error, Style::default().fg(Color::Red)),
        ])
    } else {
        Line::from(vec![
            Span::raw("  Actors: "),
            Span::styled(
                format!("{}", total),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw(" total | "),
            Span::styled(format!("{}", running), Style::default().fg(Color::Green)),
            Span::raw(" running | "),
            Span::styled(format!("{}", stopped), Style::default().fg(Color::Red)),
            Span::raw(" stopped"),
        ])
    };

    let header =
        Paragraph::new(vec![title, stats]).block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(header, area);
}

/// Render the actor table.
fn render_table(frame: &mut Frame, app: &App, area: Rect) {
    // Build header with sort indicator
    let header_cells = [
        ("ID", SortColumn::Id),
        ("Name", SortColumn::Name),
        ("Status", SortColumn::Status),
        ("Uptime", SortColumn::Uptime),
        ("Groups", SortColumn::Groups),
    ]
    .iter()
    .map(|(name, col)| {
        let mut text = name.to_string();
        if *col == app.sort_column {
            text = format!("{} {}", text, app.sort_direction.indicator());
        }
        Cell::from(text).style(Style::default().fg(Color::Yellow).bold())
    });

    let header = Row::new(header_cells).height(1).bottom_margin(1);

    // Build rows
    let rows: Vec<Row> = app
        .visible_actors()
        .iter()
        .enumerate()
        .map(|(i, actor)| {
            let status_style = match actor.status {
                ActorStatus::Running => Style::default().fg(Color::Green),
                ActorStatus::Stopped => Style::default().fg(Color::Red),
                ActorStatus::Starting => Style::default().fg(Color::Yellow),
                ActorStatus::Stopping => Style::default().fg(Color::Yellow),
                ActorStatus::Draining => Style::default().fg(Color::Magenta),
                _ => Style::default().fg(Color::Gray),
            };

            let groups_str = if actor.groups.is_empty() {
                "-".to_string()
            } else {
                actor.groups.join(", ")
            };

            let cells = vec![
                Cell::from(actor.id.clone()),
                Cell::from(actor.name.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(format!("{:?}", actor.status)).style(status_style),
                Cell::from(actor.uptime_string()),
                Cell::from(groups_str),
            ];

            let row = Row::new(cells);

            // Highlight selected row
            if i == app.selected {
                row.style(Style::default().bg(Color::DarkGray))
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Length(8),  // ID
        Constraint::Min(20),    // Name
        Constraint::Length(12), // Status
        Constraint::Length(10), // Uptime
        Constraint::Min(20),    // Groups
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Actors ")
                .title_style(Style::default().bold()),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    // Use TableState for scrolling
    let mut state = TableState::default();
    state.select(Some(app.selected));

    frame.render_stateful_widget(table, area, &mut state);
}

/// Render the status bar.
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let filter_info = if app.filter.is_empty() {
        String::new()
    } else {
        format!(" | Filter: \"{}\"", app.filter)
    };

    let refresh_secs = app.refresh_interval.as_secs();

    let status = Line::from(vec![
        Span::styled(" [q]", Style::default().fg(Color::Yellow)),
        Span::raw("uit  "),
        Span::styled("[s]", Style::default().fg(Color::Yellow)),
        Span::raw("ort  "),
        Span::styled("[S]", Style::default().fg(Color::Yellow)),
        Span::raw("reverse  "),
        Span::styled("[/]", Style::default().fg(Color::Yellow)),
        Span::raw("filter  "),
        Span::styled("[c]", Style::default().fg(Color::Yellow)),
        Span::raw("lear  "),
        Span::styled("[r]", Style::default().fg(Color::Yellow)),
        Span::raw("efresh  "),
        Span::styled("[?]", Style::default().fg(Color::Yellow)),
        Span::raw("help"),
        Span::raw(format!("{}  Refresh: {}s", filter_info, refresh_secs)),
    ]);

    let status_bar = Paragraph::new(status).block(Block::default().borders(Borders::TOP));

    frame.render_widget(status_bar, area);
}

/// Render the help overlay.
fn render_help_overlay(frame: &mut Frame) {
    let area = centered_rect(60, 70, frame.area());

    // Clear the area first
    frame.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default().bold().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q, Esc    ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  ↑/k, ↓/j  ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate up/down"),
        ]),
        Line::from(vec![
            Span::styled("  g, G      ", Style::default().fg(Color::Yellow)),
            Span::raw("Go to top/bottom"),
        ]),
        Line::from(vec![
            Span::styled("  PgUp/PgDn ", Style::default().fg(Color::Yellow)),
            Span::raw("Page up/down"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  s         ", Style::default().fg(Color::Yellow)),
            Span::raw("Cycle sort column"),
        ]),
        Line::from(vec![
            Span::styled("  S         ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle sort direction"),
        ]),
        Line::from(vec![
            Span::styled("  /         ", Style::default().fg(Color::Yellow)),
            Span::raw("Enter filter mode"),
        ]),
        Line::from(vec![
            Span::styled("  c         ", Style::default().fg(Color::Yellow)),
            Span::raw("Clear filter"),
        ]),
        Line::from(vec![
            Span::styled("  r         ", Style::default().fg(Color::Yellow)),
            Span::raw("Force refresh"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to close",
            Style::default().italic().fg(Color::Gray),
        )),
    ];

    let help = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .title_style(Style::default().bold())
            .style(Style::default().bg(Color::Black)),
    );

    frame.render_widget(help, area);
}

/// Render the filter input box.
fn render_filter_input(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 15, frame.area());

    frame.render_widget(Clear, area);

    let input = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Filter actors by name or ID:",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(&app.filter, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Yellow)), // Cursor
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter to apply, Esc to cancel",
            Style::default().italic().fg(Color::Gray),
        )),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Filter ")
            .title_style(Style::default().bold())
            .style(Style::default().bg(Color::Black)),
    );

    frame.render_widget(input, area);
}

/// Create a centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

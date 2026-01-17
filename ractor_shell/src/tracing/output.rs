//! Trace event output formatting.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Local;
use colored::Colorize;

/// A trace event captured from the tracing layer.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Timestamp of the event
    pub timestamp: chrono::DateTime<Local>,
    /// Actor ID (e.g., "0.1")
    pub actor_id: Option<String>,
    /// Actor name (if registered)
    pub actor_name: Option<String>,
    /// Event type (span enter, exit, or event)
    pub event_type: TraceEventType,
    /// Event level (trace, debug, info, warn, error)
    pub level: tracing::Level,
    /// Span/event name
    pub target: String,
    /// Event message or span name
    pub message: String,
    /// Additional fields
    pub fields: Vec<(String, String)>,
}

/// Type of trace event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventType {
    /// Span entered (actor started processing)
    SpanEnter,
    /// Span exited (actor finished processing)
    SpanExit,
    /// A tracing event (log message)
    Event,
}

impl fmt::Display for TraceEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceEventType::SpanEnter => write!(f, "ENTER"),
            TraceEventType::SpanExit => write!(f, "EXIT"),
            TraceEventType::Event => write!(f, "EVENT"),
        }
    }
}

/// Output format for trace events.
#[derive(Debug, Clone, Copy, Default)]
pub enum TraceOutputFormat {
    /// Human-readable colored output
    #[default]
    Pretty,
    /// JSON format for machine processing
    Json,
    /// Compact single-line format
    Compact,
}

/// Destination for trace output.
#[derive(Debug)]
pub enum TraceOutput {
    /// Write to stdout
    Console,
    /// Write to a file
    File {
        path: PathBuf,
        writer: Arc<Mutex<BufWriter<File>>>,
    },
}

impl TraceOutput {
    /// Create a console output.
    pub fn console() -> Self {
        TraceOutput::Console
    }

    /// Create a file output.
    pub fn file(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let writer = Arc::new(Mutex::new(BufWriter::new(file)));
        Ok(TraceOutput::File { path, writer })
    }

    /// Write a trace event.
    pub fn write(&self, event: &TraceEvent, format: TraceOutputFormat) -> io::Result<()> {
        match self {
            TraceOutput::Console => {
                let formatted = format_event(event, format, true);
                println!("{}", formatted);
                Ok(())
            }
            TraceOutput::File { writer, .. } => {
                let formatted = format_event(event, format, false);
                let mut w = writer.lock().unwrap();
                writeln!(w, "{}", formatted)?;
                w.flush()
            }
        }
    }

    /// Get the file path if this is a file output.
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            TraceOutput::Console => None,
            TraceOutput::File { path, .. } => Some(path),
        }
    }
}

/// Format a trace event for display.
fn format_event(event: &TraceEvent, format: TraceOutputFormat, use_colors: bool) -> String {
    match format {
        TraceOutputFormat::Pretty => format_pretty(event, use_colors),
        TraceOutputFormat::Json => format_json(event),
        TraceOutputFormat::Compact => format_compact(event, use_colors),
    }
}

fn format_pretty(event: &TraceEvent, use_colors: bool) -> String {
    let time = event.timestamp.format("%H:%M:%S%.3f");
    let actor = event
        .actor_name
        .as_deref()
        .or(event.actor_id.as_deref())
        .unwrap_or("?");

    let (event_symbol, event_type_str) = match event.event_type {
        TraceEventType::SpanEnter => ("→", "ENTER"),
        TraceEventType::SpanExit => ("←", "EXIT"),
        TraceEventType::Event => ("●", "EVENT"),
    };

    let level_str = match event.level {
        tracing::Level::TRACE => "TRACE",
        tracing::Level::DEBUG => "DEBUG",
        tracing::Level::INFO => "INFO",
        tracing::Level::WARN => "WARN",
        tracing::Level::ERROR => "ERROR",
    };

    let fields_str = if event.fields.is_empty() {
        String::new()
    } else {
        let fields: Vec<String> = event
            .fields
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        format!(" {{{}}}", fields.join(", "))
    };

    if use_colors {
        let event_colored = match event.event_type {
            TraceEventType::SpanEnter => event_symbol.green(),
            TraceEventType::SpanExit => event_symbol.yellow(),
            TraceEventType::Event => event_symbol.cyan(),
        };

        let level_colored = match event.level {
            tracing::Level::TRACE => level_str.bright_black(),
            tracing::Level::DEBUG => level_str.blue(),
            tracing::Level::INFO => level_str.green(),
            tracing::Level::WARN => level_str.yellow(),
            tracing::Level::ERROR => level_str.red(),
        };

        format!(
            "{} {} {} {} {} {}{}",
            format!("[{}]", time).bright_black(),
            event_colored,
            event_type_str.bright_black(),
            actor.cyan(),
            level_colored,
            event.message,
            fields_str.bright_black()
        )
    } else {
        format!(
            "[{}] {} {} {} {} {}{}",
            time, event_symbol, event_type_str, actor, level_str, event.message, fields_str
        )
    }
}

fn format_compact(event: &TraceEvent, use_colors: bool) -> String {
    let time = event.timestamp.format("%H:%M:%S");
    let actor = event
        .actor_name
        .as_deref()
        .or(event.actor_id.as_deref())
        .unwrap_or("?");

    let symbol = match event.event_type {
        TraceEventType::SpanEnter => ">",
        TraceEventType::SpanExit => "<",
        TraceEventType::Event => ".",
    };

    if use_colors {
        format!(
            "{} {} {} {}",
            format!("[{}]", time).bright_black(),
            symbol.cyan(),
            actor.cyan(),
            event.message
        )
    } else {
        format!("[{}] {} {} {}", time, symbol, actor, event.message)
    }
}

fn format_json(event: &TraceEvent) -> String {
    let mut obj = serde_json::json!({
        "timestamp": event.timestamp.to_rfc3339(),
        "event_type": event.event_type.to_string(),
        "level": event.level.to_string(),
        "target": event.target,
        "message": event.message,
    });

    if let Some(ref id) = event.actor_id {
        obj["actor_id"] = serde_json::Value::String(id.clone());
    }
    if let Some(ref name) = event.actor_name {
        obj["actor_name"] = serde_json::Value::String(name.clone());
    }
    if !event.fields.is_empty() {
        let fields: serde_json::Map<String, serde_json::Value> = event
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        obj["fields"] = serde_json::Value::Object(fields);
    }

    serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod output_tests;

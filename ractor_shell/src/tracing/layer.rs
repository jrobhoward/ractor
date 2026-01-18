//! Custom tracing layer for capturing actor events.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use chrono::Local;
use tokio::sync::mpsc;
use tracing::span::{Attributes, Id};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use super::filter::TraceFilter;
use super::output::{TraceEvent, TraceEventType, TraceOutput, TraceOutputFormat};

/// Handle for controlling the tracing layer at runtime.
#[derive(Clone)]
pub struct TracingHandle {
    inner: Arc<TracingHandleInner>,
}

struct TracingHandleInner {
    /// Filter for which actors to trace
    filter: RwLock<TraceFilter>,
    /// Output destinations (console, files)
    outputs: RwLock<Vec<(TraceOutput, TraceOutputFormat)>>,
    /// Whether tracing is enabled
    enabled: AtomicBool,
    /// Optional channel to send trace events (for remote tracing subscriptions)
    event_sender: RwLock<Option<mpsc::UnboundedSender<TraceEvent>>>,
}

impl TracingHandle {
    /// Create a new tracing handle.
    fn new() -> Self {
        Self {
            inner: Arc::new(TracingHandleInner {
                filter: RwLock::new(TraceFilter::new()),
                outputs: RwLock::new(vec![(TraceOutput::console(), TraceOutputFormat::Pretty)]),
                enabled: AtomicBool::new(false),
                event_sender: RwLock::new(None),
            }),
        }
    }

    /// Enable tracing with a pattern.
    pub fn trace(&self, pattern: &str) {
        let mut filter = self.inner.filter.write().unwrap();
        filter.add_pattern(pattern);
        self.inner.enabled.store(true, Ordering::SeqCst);
    }

    /// Disable tracing for a pattern.
    pub fn untrace(&self, pattern: &str) -> bool {
        let mut filter = self.inner.filter.write().unwrap();
        let removed = filter.remove_pattern(pattern);
        if !filter.is_active() {
            self.inner.enabled.store(false, Ordering::SeqCst);
        }
        removed
    }

    /// Stop all tracing.
    pub fn trace_off(&self) {
        let mut filter = self.inner.filter.write().unwrap();
        filter.clear();
        self.inner.enabled.store(false, Ordering::SeqCst);
    }

    /// Check if tracing is active.
    pub fn is_active(&self) -> bool {
        self.inner.enabled.load(Ordering::SeqCst)
    }

    /// Get active trace patterns.
    pub fn patterns(&self) -> Vec<String> {
        let filter = self.inner.filter.read().unwrap();
        filter.patterns().iter().map(|s| s.to_string()).collect()
    }

    /// Add a file output.
    pub fn add_file_output(
        &self,
        path: std::path::PathBuf,
        format: TraceOutputFormat,
    ) -> std::io::Result<()> {
        let output = TraceOutput::file(path)?;
        let mut outputs = self.inner.outputs.write().unwrap();
        outputs.push((output, format));
        Ok(())
    }

    /// Remove file outputs (keeps console).
    pub fn clear_file_outputs(&self) {
        let mut outputs = self.inner.outputs.write().unwrap();
        outputs.retain(|(o, _)| matches!(o, TraceOutput::Console));
    }

    /// Get list of output destinations.
    pub fn outputs(&self) -> Vec<String> {
        let outputs = self.inner.outputs.read().unwrap();
        outputs
            .iter()
            .map(|(o, _)| match o {
                TraceOutput::Console => "console".to_string(),
                TraceOutput::File { path, .. } => path.display().to_string(),
            })
            .collect()
    }

    /// Set an event sender for remote tracing subscriptions.
    /// Events will be sent through this channel in addition to normal outputs.
    pub fn set_event_sender(&self, sender: mpsc::UnboundedSender<TraceEvent>) {
        let mut event_sender = self.inner.event_sender.write().unwrap();
        *event_sender = Some(sender);
    }

    /// Remove the event sender.
    pub fn clear_event_sender(&self) {
        let mut event_sender = self.inner.event_sender.write().unwrap();
        *event_sender = None;
    }

    /// Check if an actor matches the current filter.
    fn matches(&self, actor_name: Option<&str>, actor_id: Option<&str>) -> bool {
        if !self.inner.enabled.load(Ordering::SeqCst) {
            return false;
        }
        let filter = self.inner.filter.read().unwrap();
        filter.matches(actor_name) || actor_id.map(|id| filter.matches_id(id)).unwrap_or(false)
    }

    /// Check if a target (module path) matches the current filter.
    fn matches_target(&self, target: &str) -> bool {
        if !self.inner.enabled.load(Ordering::SeqCst) {
            return false;
        }
        let filter = self.inner.filter.read().unwrap();
        filter.matches_target(target)
    }

    /// Write an event to all outputs.
    fn write_event(&self, event: TraceEvent) {
        // Write to configured outputs (console, files)
        let outputs = self.inner.outputs.read().unwrap();
        for (output, format) in outputs.iter() {
            let _ = output.write(&event, *format);
        }
        drop(outputs); // Release lock before sending to channel

        // Send to event channel for remote tracing if configured
        let event_sender = self.inner.event_sender.read().unwrap();
        if let Some(sender) = event_sender.as_ref() {
            let _ = sender.send(event.clone());
        }
    }
}

/// A tracing layer that captures actor events for the shell.
pub struct ShellTracingLayer {
    handle: TracingHandle,
}

impl ShellTracingLayer {
    /// Create a new shell tracing layer and return both the layer and a handle.
    pub fn new() -> (Self, TracingHandle) {
        let handle = TracingHandle::new();
        let layer = Self {
            handle: handle.clone(),
        };
        (layer, handle)
    }
}

impl Default for ShellTracingLayer {
    fn default() -> Self {
        Self::new().0
    }
}

/// Extension data stored on each span.
#[derive(Debug, Default)]
struct SpanData {
    actor_id: Option<String>,
    actor_name: Option<String>,
    is_actor_span: bool,
}

/// Visitor for extracting fields from spans and events.
struct FieldVisitor {
    actor_id: Option<String>,
    actor_name: Option<String>,
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            actor_id: None,
            actor_name: None,
            message: None,
            fields: Vec::new(),
        }
    }
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let value_str = format!("{:?}", value);

        match name {
            "id" => self.actor_id = Some(value_str.trim_matches('"').to_string()),
            "name" => {
                // Handle Option<String> format: Some("name") or None
                let trimmed = value_str.trim_matches('"');
                if trimmed.starts_with("Some(") && trimmed.ends_with(')') {
                    let inner = &trimmed[5..trimmed.len() - 1];
                    self.actor_name = Some(inner.trim_matches('"').to_string());
                } else if trimmed != "None" {
                    self.actor_name = Some(trimmed.to_string());
                }
            }
            "message" => self.message = Some(value_str.trim_matches('"').to_string()),
            _ => self.fields.push((name.to_string(), value_str)),
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        match name {
            "id" => self.actor_id = Some(value.to_string()),
            "name" => self.actor_name = Some(value.to_string()),
            "message" => self.message = Some(value.to_string()),
            _ => self.fields.push((name.to_string(), value.to_string())),
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }
}

impl<S> Layer<S> for ShellTracingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        // Only process "Actor" spans from ractor
        if attrs.metadata().name() != "Actor" {
            return;
        }

        let mut visitor = FieldVisitor::new();
        attrs.record(&mut visitor);

        // Check if this actor matches our filter
        if !self
            .handle
            .matches(visitor.actor_name.as_deref(), visitor.actor_id.as_deref())
        {
            return;
        }

        // Store span data for later use
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            extensions.insert(SpanData {
                actor_id: visitor.actor_id,
                actor_name: visitor.actor_name,
                is_actor_span: true,
            });
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if !self.handle.is_active() {
            return;
        }

        if let Some(span) = ctx.span(id) {
            let extensions = span.extensions();
            if let Some(data) = extensions.get::<SpanData>() {
                if data.is_actor_span {
                    let event = TraceEvent {
                        timestamp: Local::now(),
                        actor_id: data.actor_id.clone(),
                        actor_name: data.actor_name.clone(),
                        event_type: TraceEventType::SpanEnter,
                        level: *span.metadata().level(),
                        target: span.metadata().target().to_string(),
                        message: span.metadata().name().to_string(),
                        fields: vec![],
                    };
                    self.handle.write_event(event);
                }
            }
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if !self.handle.is_active() {
            return;
        }

        if let Some(span) = ctx.span(id) {
            let extensions = span.extensions();
            if let Some(data) = extensions.get::<SpanData>() {
                if data.is_actor_span {
                    let event = TraceEvent {
                        timestamp: Local::now(),
                        actor_id: data.actor_id.clone(),
                        actor_name: data.actor_name.clone(),
                        event_type: TraceEventType::SpanExit,
                        level: *span.metadata().level(),
                        target: span.metadata().target().to_string(),
                        message: span.metadata().name().to_string(),
                        fields: vec![],
                    };
                    self.handle.write_event(event);
                }
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if !self.handle.is_active() {
            return;
        }

        // Skip events from the tracing infrastructure itself to prevent infinite recursion
        let target = event.metadata().target();
        if target.starts_with("ractor_shell::tracing")
            || target.starts_with("ractor_shell::introspection")
        {
            return;
        }

        // Look for parent actor span to get actor context
        let mut actor_id = None;
        let mut actor_name = None;

        if let Some(scope) = ctx.event_scope(event) {
            for span in scope {
                let extensions = span.extensions();
                if let Some(data) = extensions.get::<SpanData>() {
                    if data.is_actor_span {
                        actor_id = data.actor_id.clone();
                        actor_name = data.actor_name.clone();
                        break;
                    }
                }
            }
        }

        // If not in an actor span, check if the event message contains actor info
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        // Use event's actor info if available
        if actor_id.is_none() {
            actor_id = visitor.actor_id;
        }
        if actor_name.is_none() {
            actor_name = visitor.actor_name;
        }

        // Only emit if actor name, actor id, or target matches our filter
        let target = event.metadata().target();
        let matches = self
            .handle
            .matches(actor_name.as_deref(), actor_id.as_deref())
            || self.handle.matches_target(target);

        if !matches {
            return;
        }

        let message = visitor
            .message
            .unwrap_or_else(|| event.metadata().name().to_string());

        let trace_event = TraceEvent {
            timestamp: Local::now(),
            actor_id: actor_id.clone(),
            actor_name: actor_name.clone(),
            event_type: TraceEventType::Event,
            level: *event.metadata().level(),
            target: event.metadata().target().to_string(),
            message,
            fields: visitor.fields,
        };

        self.handle.write_event(trace_event);
    }
}

#[cfg(test)]
mod layer_tests;

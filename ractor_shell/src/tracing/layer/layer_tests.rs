//! Tests for ShellTracingLayer
#![allow(non_snake_case)]

use super::*;
use tokio::sync::mpsc;

// ==================== ShellTracingLayer Creation Tests ====================

#[test]
fn ShellTracingLayer___new___returns_layer_and_handle() {
    let (layer, handle) = ShellTracingLayer::new();

    // Layer exists (we can't inspect it much, but we can check handle)
    assert!(!handle.is_active());
    // Verify the layer has the same handle by testing shared state
    handle.trace("test_*");
    assert!(handle.is_active());
    drop(layer); // Just to use it
}

#[test]
fn ShellTracingLayer___default___creates_layer() {
    let layer = ShellTracingLayer::default();

    // Default layer exists - we can't access its handle directly
    // but we verify it doesn't panic and creates a valid layer
    drop(layer);
}

// ==================== TracingHandle Clone Tests ====================

#[test]
fn TracingHandle___clone___shares_state() {
    let (_, handle1) = ShellTracingLayer::new();
    let handle2 = handle1.clone();

    // Modify through handle1
    handle1.trace("pattern_a");

    // Should be visible through handle2
    assert!(handle2.is_active());
    assert!(handle2.patterns().contains(&"pattern_a".to_string()));
}

#[test]
fn TracingHandle___clone___modifications_sync() {
    let (_, handle1) = ShellTracingLayer::new();
    let handle2 = handle1.clone();

    handle1.trace("pattern_*");
    handle2.trace("other_*");

    let patterns = handle1.patterns();
    assert_eq!(patterns.len(), 2);
    assert!(patterns.contains(&"pattern_*".to_string()));
    assert!(patterns.contains(&"other_*".to_string()));
}

// ==================== TracingHandle Basic Tests ====================

#[test]
fn tracing_handle_new___not_active_by_default() {
    let (_, handle) = ShellTracingLayer::new();

    assert!(!handle.is_active());
    assert!(handle.patterns().is_empty());
}

#[test]
fn tracing_handle_trace___activates_tracing() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("worker_*");

    assert!(handle.is_active());
    assert_eq!(handle.patterns(), vec!["worker_*"]);
}

#[test]
fn tracing_handle_trace___multiple_patterns() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("worker_*");
    handle.trace("supervisor");

    assert!(handle.is_active());

    let patterns = handle.patterns();
    assert_eq!(patterns.len(), 2);
    assert!(patterns.contains(&"worker_*".to_string()));
    assert!(patterns.contains(&"supervisor".to_string()));
}

#[test]
fn tracing_handle_trace___wildcard_all___activates() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("*");

    assert!(handle.is_active());
    assert_eq!(handle.patterns(), vec!["*"]);
}

#[test]
fn tracing_handle_trace___empty_pattern___still_activates() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("");

    // Empty pattern behavior depends on TraceFilter implementation
    assert!(handle.is_active());
}

#[test]
fn tracing_handle_untrace___removes_pattern() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("worker_*");
    handle.trace("supervisor");

    assert!(handle.untrace("worker_*"));
    assert!(handle.is_active()); // Still has supervisor

    let patterns = handle.patterns();
    assert_eq!(patterns.len(), 1);
    assert!(patterns.contains(&"supervisor".to_string()));
}

#[test]
fn tracing_handle_untrace___last_pattern___deactivates() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("worker_*");
    handle.untrace("worker_*");

    assert!(!handle.is_active());
}

#[test]
fn tracing_handle_untrace___nonexistent_pattern___returns_false() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("worker_*");

    assert!(!handle.untrace("nonexistent"));
    assert!(handle.is_active()); // Still active
    assert_eq!(handle.patterns().len(), 1);
}

#[test]
fn tracing_handle_trace_off___clears_all() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("worker_*");
    handle.trace("supervisor");
    handle.trace_off();

    assert!(!handle.is_active());
    assert!(handle.patterns().is_empty());
}

#[test]
fn tracing_handle_trace_off___when_already_inactive___safe() {
    let (_, handle) = ShellTracingLayer::new();

    // Should be safe to call even when not active
    handle.trace_off();

    assert!(!handle.is_active());
    assert!(handle.patterns().is_empty());
}

// ==================== Output Tests ====================

#[test]
fn tracing_handle_outputs___default_is_console() {
    let (_, handle) = ShellTracingLayer::new();

    let outputs = handle.outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0], "console");
}

#[test]
fn tracing_handle_add_file_output___adds_to_outputs() {
    let (_, handle) = ShellTracingLayer::new();

    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_layer_output.log");
    let _ = std::fs::remove_file(&path);

    handle
        .add_file_output(path.clone(), TraceOutputFormat::Json)
        .unwrap();

    let outputs = handle.outputs();
    assert_eq!(outputs.len(), 2);
    assert!(outputs.contains(&"console".to_string()));
    assert!(outputs.iter().any(|o| o.contains("test_layer_output.log")));

    // Cleanup
    let _ = std::fs::remove_file(path);
}

#[test]
fn tracing_handle_add_file_output___multiple_files() {
    let (_, handle) = ShellTracingLayer::new();

    let temp_dir = std::env::temp_dir();
    let path1 = temp_dir.join("test_layer_multi1.log");
    let path2 = temp_dir.join("test_layer_multi2.log");
    let _ = std::fs::remove_file(&path1);
    let _ = std::fs::remove_file(&path2);

    handle
        .add_file_output(path1.clone(), TraceOutputFormat::Json)
        .unwrap();
    handle
        .add_file_output(path2.clone(), TraceOutputFormat::Pretty)
        .unwrap();

    let outputs = handle.outputs();
    assert_eq!(outputs.len(), 3);
    assert!(outputs.contains(&"console".to_string()));

    // Cleanup
    let _ = std::fs::remove_file(path1);
    let _ = std::fs::remove_file(path2);
}

#[test]
fn tracing_handle_clear_file_outputs___keeps_console() {
    let (_, handle) = ShellTracingLayer::new();

    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_layer_clear.log");
    let _ = std::fs::remove_file(&path);

    handle
        .add_file_output(path.clone(), TraceOutputFormat::Compact)
        .unwrap();
    handle.clear_file_outputs();

    let outputs = handle.outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0], "console");

    // Cleanup
    let _ = std::fs::remove_file(path);
}

#[test]
fn tracing_handle_clear_file_outputs___multiple_files___clears_all() {
    let (_, handle) = ShellTracingLayer::new();

    let temp_dir = std::env::temp_dir();
    let path1 = temp_dir.join("test_clear_multi1.log");
    let path2 = temp_dir.join("test_clear_multi2.log");
    let _ = std::fs::remove_file(&path1);
    let _ = std::fs::remove_file(&path2);

    handle
        .add_file_output(path1.clone(), TraceOutputFormat::Json)
        .unwrap();
    handle
        .add_file_output(path2.clone(), TraceOutputFormat::Pretty)
        .unwrap();
    handle.clear_file_outputs();

    let outputs = handle.outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0], "console");

    // Cleanup
    let _ = std::fs::remove_file(path1);
    let _ = std::fs::remove_file(path2);
}

// ==================== Min Level Tests ====================

#[test]
fn tracing_handle_min_level___default_is_trace() {
    let (_, handle) = ShellTracingLayer::new();

    assert_eq!(handle.min_level(), MinLevel::Trace);
}

#[test]
fn tracing_handle_set_min_level___updates_level() {
    let (_, handle) = ShellTracingLayer::new();

    handle.set_min_level(MinLevel::Info);

    assert_eq!(handle.min_level(), MinLevel::Info);
}

#[test]
fn tracing_handle_set_min_level___all_levels() {
    let (_, handle) = ShellTracingLayer::new();

    handle.set_min_level(MinLevel::Trace);
    assert_eq!(handle.min_level(), MinLevel::Trace);

    handle.set_min_level(MinLevel::Debug);
    assert_eq!(handle.min_level(), MinLevel::Debug);

    handle.set_min_level(MinLevel::Info);
    assert_eq!(handle.min_level(), MinLevel::Info);

    handle.set_min_level(MinLevel::Warn);
    assert_eq!(handle.min_level(), MinLevel::Warn);

    handle.set_min_level(MinLevel::Error);
    assert_eq!(handle.min_level(), MinLevel::Error);
}

#[test]
fn tracing_handle_level_allowed___respects_min_level() {
    let (_, handle) = ShellTracingLayer::new();
    handle.set_min_level(MinLevel::Warn);

    assert!(!handle.level_allowed(tracing::Level::TRACE));
    assert!(!handle.level_allowed(tracing::Level::DEBUG));
    assert!(!handle.level_allowed(tracing::Level::INFO));
    assert!(handle.level_allowed(tracing::Level::WARN));
    assert!(handle.level_allowed(tracing::Level::ERROR));
}

#[test]
fn tracing_handle_level_allowed___trace_level___allows_all() {
    let (_, handle) = ShellTracingLayer::new();
    handle.set_min_level(MinLevel::Trace);

    assert!(handle.level_allowed(tracing::Level::TRACE));
    assert!(handle.level_allowed(tracing::Level::DEBUG));
    assert!(handle.level_allowed(tracing::Level::INFO));
    assert!(handle.level_allowed(tracing::Level::WARN));
    assert!(handle.level_allowed(tracing::Level::ERROR));
}

#[test]
fn tracing_handle_level_allowed___error_level___only_error() {
    let (_, handle) = ShellTracingLayer::new();
    handle.set_min_level(MinLevel::Error);

    assert!(!handle.level_allowed(tracing::Level::TRACE));
    assert!(!handle.level_allowed(tracing::Level::DEBUG));
    assert!(!handle.level_allowed(tracing::Level::INFO));
    assert!(!handle.level_allowed(tracing::Level::WARN));
    assert!(handle.level_allowed(tracing::Level::ERROR));
}

#[test]
fn tracing_handle_trace_off___resets_min_level() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("*");
    handle.set_min_level(MinLevel::Error);

    handle.trace_off();

    assert_eq!(handle.min_level(), MinLevel::Trace);
}

// ==================== Event Sender Tests ====================

#[test]
fn tracing_handle_set_event_sender___stores_sender() {
    let (_, handle) = ShellTracingLayer::new();
    let (tx, _rx) = mpsc::unbounded_channel::<TraceEvent>();

    handle.set_event_sender(tx);

    // We can't directly inspect the sender, but we can verify no panic
    // The real test is in write_event which sends to this channel
}

#[test]
fn tracing_handle_clear_event_sender___removes_sender() {
    let (_, handle) = ShellTracingLayer::new();
    let (tx, _rx) = mpsc::unbounded_channel::<TraceEvent>();

    handle.set_event_sender(tx);
    handle.clear_event_sender();

    // Verify clear doesn't panic and can be called safely
}

#[test]
fn tracing_handle_clear_event_sender___when_not_set___safe() {
    let (_, handle) = ShellTracingLayer::new();

    // Should be safe to clear even when no sender was set
    handle.clear_event_sender();
}

#[test]
fn tracing_handle_set_event_sender___replace_existing() {
    let (_, handle) = ShellTracingLayer::new();
    let (tx1, _rx1) = mpsc::unbounded_channel::<TraceEvent>();
    let (tx2, _rx2) = mpsc::unbounded_channel::<TraceEvent>();

    handle.set_event_sender(tx1);
    handle.set_event_sender(tx2); // Replace

    // Should not panic, new sender replaces old
}

// ==================== Matches Tests ====================

#[test]
fn tracing_handle_matches___when_disabled___returns_false() {
    let (_, handle) = ShellTracingLayer::new();

    // Not active, so matches should return false
    assert!(!handle.matches(Some("any_actor"), None));
    assert!(!handle.matches(None, Some("0.1")));
}

#[test]
fn tracing_handle_matches___by_name___returns_true() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("worker_*");

    assert!(handle.matches(Some("worker_1"), None));
    assert!(handle.matches(Some("worker_abc"), None));
    assert!(!handle.matches(Some("supervisor"), None));
}

#[test]
fn tracing_handle_matches___by_id___returns_true() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("0.1");

    assert!(handle.matches(None, Some("0.1")));
    assert!(!handle.matches(None, Some("0.2")));
}

#[test]
fn tracing_handle_matches___wildcard___matches_all() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("*");

    assert!(handle.matches(Some("any_actor"), None));
    assert!(handle.matches(Some("another"), None));
    assert!(handle.matches(None, Some("0.1")));
}

#[test]
fn tracing_handle_matches___none_values___returns_false() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("specific_actor");

    assert!(!handle.matches(None, None));
}

// ==================== Matches Target Tests ====================

#[test]
fn tracing_handle_matches_target___when_disabled___returns_false() {
    let (_, handle) = ShellTracingLayer::new();

    assert!(!handle.matches_target("any::target"));
}

#[test]
fn tracing_handle_matches_target___exact_match___returns_true() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("my_module::submodule");

    assert!(handle.matches_target("my_module::submodule"));
}

#[test]
fn tracing_handle_matches_target___wildcard___returns_true() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("my_module::*");

    assert!(handle.matches_target("my_module::foo"));
    assert!(handle.matches_target("my_module::bar"));
}

// ==================== Matches Any Field Tests ====================

#[test]
fn tracing_handle_matches_any_field___when_disabled___returns_false() {
    let (_, handle) = ShellTracingLayer::new();

    let fields = vec![("key".to_string(), "value".to_string())];
    assert!(!handle.matches_any_field(&fields));
}

#[test]
fn tracing_handle_matches_any_field___matching_value___returns_true() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("special_value");

    let fields = vec![("key".to_string(), "special_value".to_string())];
    assert!(handle.matches_any_field(&fields));
}

#[test]
fn tracing_handle_matches_any_field___no_matching_value___returns_false() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("something_else");

    let fields = vec![("key".to_string(), "different_value".to_string())];
    assert!(!handle.matches_any_field(&fields));
}

#[test]
fn tracing_handle_matches_any_field___empty_fields___with_wildcard___returns_true() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("*");

    // Wildcard pattern matches even with empty fields
    let fields: Vec<(String, String)> = vec![];
    assert!(handle.matches_any_field(&fields));
}

#[test]
fn tracing_handle_matches_any_field___empty_fields___specific_pattern___returns_false() {
    let (_, handle) = ShellTracingLayer::new();
    handle.trace("specific_pattern");

    // Specific pattern doesn't match empty fields
    let fields: Vec<(String, String)> = vec![];
    assert!(!handle.matches_any_field(&fields));
}

// ==================== SpanData Tests ====================

#[test]
fn SpanData___default___has_none_values() {
    let data = SpanData::default();

    assert!(data.actor_id.is_none());
    assert!(data.actor_name.is_none());
    assert!(!data.is_actor_span);
}

#[test]
fn SpanData___debug___formats_correctly() {
    let data = SpanData {
        actor_id: Some("0.1".to_string()),
        actor_name: Some("test_actor".to_string()),
        is_actor_span: true,
    };

    let debug_str = format!("{:?}", data);
    assert!(debug_str.contains("0.1"));
    assert!(debug_str.contains("test_actor"));
    assert!(debug_str.contains("true"));
}

// ==================== FieldVisitor Tests ====================

#[test]
fn FieldVisitor___new___has_empty_fields() {
    let visitor = FieldVisitor::new();

    assert!(visitor.actor_id.is_none());
    assert!(visitor.actor_name.is_none());
    assert!(visitor.message.is_none());
    assert!(visitor.fields.is_empty());
}

// Note: FieldVisitor::record_* methods cannot be directly tested because
// tracing::field::Field::new is not public. These methods are tested
// indirectly through integration tests with actual tracing spans and events.

// ==================== Write Event Tests ====================

#[tokio::test]
async fn tracing_handle_write_event___sends_to_channel() {
    let (_, handle) = ShellTracingLayer::new();
    let (tx, mut rx) = mpsc::unbounded_channel::<TraceEvent>();

    handle.set_event_sender(tx);

    let event = TraceEvent {
        timestamp: chrono::Local::now(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some("test_actor".to_string()),
        event_type: TraceEventType::Event,
        level: tracing::Level::INFO,
        target: "test::target".to_string(),
        message: "test message".to_string(),
        fields: vec![],
    };

    handle.write_event(event.clone());

    // Check if event was sent to channel
    let received = rx.try_recv();
    assert!(received.is_ok());
    let received_event = received.unwrap();
    assert_eq!(received_event.message, "test message");
    assert_eq!(received_event.actor_id, Some("0.1".to_string()));
}

#[tokio::test]
async fn tracing_handle_write_event___without_sender___no_panic() {
    let (_, handle) = ShellTracingLayer::new();

    let event = TraceEvent {
        timestamp: chrono::Local::now(),
        actor_id: None,
        actor_name: None,
        event_type: TraceEventType::Event,
        level: tracing::Level::DEBUG,
        target: "test".to_string(),
        message: "test".to_string(),
        fields: vec![],
    };

    // Should not panic even without event sender
    handle.write_event(event);
}

#[tokio::test]
async fn tracing_handle_write_event___after_clear_sender___no_panic() {
    let (_, handle) = ShellTracingLayer::new();
    let (tx, _rx) = mpsc::unbounded_channel::<TraceEvent>();

    handle.set_event_sender(tx);
    handle.clear_event_sender();

    let event = TraceEvent {
        timestamp: chrono::Local::now(),
        actor_id: None,
        actor_name: None,
        event_type: TraceEventType::SpanEnter,
        level: tracing::Level::TRACE,
        target: "test".to_string(),
        message: "test".to_string(),
        fields: vec![],
    };

    // Should not panic after clearing sender
    handle.write_event(event);
}

// ==================== Integration-style Tests ====================

#[test]
fn tracing_workflow___typical_usage___works_correctly() {
    let (layer, handle) = ShellTracingLayer::new();

    // Initial state
    assert!(!handle.is_active());

    // Enable tracing for specific actors
    handle.trace("worker_*");
    handle.trace("supervisor");
    assert!(handle.is_active());
    assert_eq!(handle.patterns().len(), 2);

    // Set level filter
    handle.set_min_level(MinLevel::Info);
    assert_eq!(handle.min_level(), MinLevel::Info);

    // Disable one pattern
    handle.untrace("supervisor");
    assert!(handle.is_active());
    assert_eq!(handle.patterns().len(), 1);

    // Turn off all tracing
    handle.trace_off();
    assert!(!handle.is_active());
    assert!(handle.patterns().is_empty());
    assert_eq!(handle.min_level(), MinLevel::Trace); // Reset

    drop(layer);
}

#[test]
fn tracing_workflow___file_output___works_correctly() {
    let (_, handle) = ShellTracingLayer::new();

    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_workflow.log");
    let _ = std::fs::remove_file(&path);

    // Add file output
    handle
        .add_file_output(path.clone(), TraceOutputFormat::Json)
        .unwrap();
    assert_eq!(handle.outputs().len(), 2);

    // Clear file outputs
    handle.clear_file_outputs();
    assert_eq!(handle.outputs().len(), 1);
    assert_eq!(handle.outputs()[0], "console");

    // Cleanup
    let _ = std::fs::remove_file(path);
}

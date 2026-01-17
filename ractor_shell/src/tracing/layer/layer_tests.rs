//! Tests for ShellTracingLayer
#![allow(non_snake_case)]

use super::*;

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
fn tracing_handle_trace_off___clears_all() {
    let (_, handle) = ShellTracingLayer::new();

    handle.trace("worker_*");
    handle.trace("supervisor");
    handle.trace_off();

    assert!(!handle.is_active());
    assert!(handle.patterns().is_empty());
}

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

// Note: FieldVisitor tests removed because tracing::field::Field::new is not public.
// The FieldVisitor implementation is tested indirectly through integration tests
// with actual tracing spans and events.

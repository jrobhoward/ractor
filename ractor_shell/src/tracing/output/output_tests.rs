//! Tests for TraceOutput
#![allow(non_snake_case)]

use super::*;

#[test]
fn trace_event_type_display___formats_correctly() {
    assert_eq!(TraceEventType::SpanEnter.to_string(), "ENTER");
    assert_eq!(TraceEventType::SpanExit.to_string(), "EXIT");
    assert_eq!(TraceEventType::Event.to_string(), "EVENT");
}

#[test]
fn format_json___includes_all_fields() {
    let event = TraceEvent {
        timestamp: Local::now(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some("worker_1".to_string()),
        event_type: TraceEventType::Event,
        level: tracing::Level::INFO,
        target: "ractor".to_string(),
        message: "Test message".to_string(),
        fields: vec![("key".to_string(), "value".to_string())],
    };

    let json = format_json(&event);

    assert!(json.contains("\"actor_id\":\"0.1\""));
    assert!(json.contains("\"actor_name\":\"worker_1\""));
    assert!(json.contains("\"event_type\":\"EVENT\""));
    assert!(json.contains("\"level\":\"INFO\""));
    assert!(json.contains("\"message\":\"Test message\""));
    assert!(json.contains("\"key\":\"value\""));
}

#[test]
fn format_json___omits_none_fields() {
    let event = TraceEvent {
        timestamp: Local::now(),
        actor_id: None,
        actor_name: None,
        event_type: TraceEventType::SpanEnter,
        level: tracing::Level::DEBUG,
        target: "ractor".to_string(),
        message: "Actor".to_string(),
        fields: vec![],
    };

    let json = format_json(&event);

    assert!(!json.contains("actor_id"));
    assert!(!json.contains("actor_name"));
    assert!(!json.contains("fields"));
}

#[test]
fn format_compact___single_line() {
    let event = TraceEvent {
        timestamp: Local::now(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some("worker_1".to_string()),
        event_type: TraceEventType::Event,
        level: tracing::Level::INFO,
        target: "ractor".to_string(),
        message: "Test".to_string(),
        fields: vec![],
    };

    let output = format_compact(&event, false);

    assert!(!output.contains('\n'));
    assert!(output.contains("worker_1"));
    assert!(output.contains("Test"));
}

#[test]
fn format_pretty___includes_timestamp_and_actor() {
    let event = TraceEvent {
        timestamp: Local::now(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some("worker_1".to_string()),
        event_type: TraceEventType::SpanEnter,
        level: tracing::Level::INFO,
        target: "ractor".to_string(),
        message: "Actor".to_string(),
        fields: vec![],
    };

    let output = format_pretty(&event, false);

    assert!(output.contains("worker_1"));
    assert!(output.contains("ENTER"));
    assert!(output.contains("INFO"));
}

#[test]
fn trace_output_console___path_is_none() {
    let output = TraceOutput::console();
    assert!(output.path().is_none());
}

#[test]
fn trace_output_file___path_is_some() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_trace.log");

    let output = TraceOutput::file(path.clone()).unwrap();

    assert_eq!(output.path(), Some(&path));

    // Cleanup
    let _ = std::fs::remove_file(path);
}

#[test]
fn trace_output_file___creates_file() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_trace_create.log");

    // Ensure file doesn't exist
    let _ = std::fs::remove_file(&path);

    let _output = TraceOutput::file(path.clone()).unwrap();

    assert!(path.exists());

    // Cleanup
    let _ = std::fs::remove_file(path);
}

#[test]
fn trace_output_file___write_event___appends_to_file() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_trace_write.log");
    let _ = std::fs::remove_file(&path);

    let output = TraceOutput::file(path.clone()).unwrap();

    let event = TraceEvent {
        timestamp: Local::now(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some("test".to_string()),
        event_type: TraceEventType::Event,
        level: tracing::Level::INFO,
        target: "test".to_string(),
        message: "Test message".to_string(),
        fields: vec![],
    };

    output.write(&event, TraceOutputFormat::Compact).unwrap();

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("Test message"));

    // Cleanup
    let _ = std::fs::remove_file(path);
}

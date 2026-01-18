//! Tests for ShellProtocolMessage
#![allow(non_snake_case)]

use super::*;
use ractor::SchemaProvider;

#[test]
fn schema_provider___message_schema___returns_valid_json() {
    let schema = ShellProtocolMessage::message_schema();
    let parsed: serde_json::Value =
        serde_json::from_str(schema).expect("Schema should be valid JSON");
    assert!(
        parsed.get("variants").is_some(),
        "Schema should have variants"
    );

    let variants = parsed.get("variants").unwrap();
    assert!(
        variants.get("StopActor").is_some(),
        "Should have StopActor variant"
    );
    assert!(variants.get("Ping").is_some(), "Should have Ping variant");
}

#[test]
fn schema_provider___from_json_stop_actor___returns_error_as_rpc() {
    // StopActor is now an RPC variant (has RpcReplyPort), so from_json should return error
    let json = serde_json::json!({"0": "test_actor"});
    let result = ShellProtocolMessage::from_json("StopActor", json);
    assert!(result.is_err(), "RPC variants should return error");
    assert!(
        result.unwrap_err().message.contains("RPC variant"),
        "Error should mention RPC variant"
    );
}

#[test]
fn schema_provider___from_json_rpc_variant___returns_error() {
    // RPC variants should return an error (they need RpcReplyPort)
    let json = serde_json::json!({});
    let result = ShellProtocolMessage::from_json("Ping", json);
    assert!(result.is_err(), "RPC variants should return error");
    assert!(
        result.unwrap_err().message.contains("RPC variant"),
        "Error should mention RPC variant"
    );
}

#[test]
fn schema_provider___from_json_unknown_variant___returns_error() {
    let json = serde_json::json!({});
    let result = ShellProtocolMessage::from_json("UnknownVariant", json);
    assert!(result.is_err(), "Unknown variant should return error");
}

#[test]
fn schema_provider___to_json___serializes_cast_variant() {
    // TraceEventNotification is the only non-RPC (cast) variant left
    let event = SerializableTraceEvent {
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some("test_actor".to_string()),
        event_type: "Event".to_string(),
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "test message".to_string(),
        fields: vec![],
    };
    let msg = ShellProtocolMessage::TraceEventNotification(event);
    let json = msg.to_json();

    assert_eq!(json.get("variant").unwrap(), "TraceEventNotification");
    // The event is serialized as field "0"
    assert!(json.get("0").is_some(), "Should have event as field 0");
}

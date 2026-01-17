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
fn schema_provider___from_json_cast_variant___deserializes_correctly() {
    // StopActor is the only non-RPC (cast) variant
    let json = serde_json::json!({"0": "test_actor"});
    let msg =
        ShellProtocolMessage::from_json("StopActor", json).expect("Should deserialize StopActor");

    match msg {
        ShellProtocolMessage::StopActor(name) => {
            assert_eq!(name, "test_actor");
        }
        _ => panic!("Expected StopActor variant"),
    }
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
    let msg = ShellProtocolMessage::StopActor("my_actor".to_string());
    let json = msg.to_json();

    assert_eq!(json.get("variant").unwrap(), "StopActor");
    assert_eq!(json.get("0").unwrap(), "my_actor");
}

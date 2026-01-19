//! Tests for schema registry
#![allow(non_snake_case)]

use super::*;

#[test]
fn has_schema___unregistered_actor___returns_false() {
    assert!(!has_schema("nonexistent_actor_12345"));
}

#[test]
fn get_schema___unregistered_actor___returns_none() {
    assert!(get_schema("nonexistent_actor_12345").is_none());
}

#[test]
fn list_schemas___returns_valid_list() {
    // Just verify it doesn't panic and returns a valid list
    let schemas = list_schemas();
    // Vec is always valid, just ensure we can iterate it
    for (name, _schema) in &schemas {
        assert!(!name.is_empty());
    }
}

#[test]
fn format_schema___valid_schema_show_all___formats_all_variants() {
    let schema = serde_json::json!({
        "variants": {
            "Ping": {
                "fields": {},
                "rpc": false
            },
            "GetValue": {
                "fields": {"0": "String"},
                "rpc": true,
                "reply_type": "i32"
            }
        }
    });

    let output = format_schema(&schema, true);
    assert!(output.contains("Ping (Cast)"));
    assert!(output.contains("GetValue (RPC)"));
    assert!(output.contains("returns: i32"));
}

#[test]
fn format_schema___valid_schema_rpc_only___filters_non_rpc_variants() {
    let schema = serde_json::json!({
        "variants": {
            "Ping": {
                "fields": {},
                "rpc": false
            },
            "GetValue": {
                "fields": {"0": "String"},
                "rpc": true,
                "reply_type": "i32"
            }
        }
    });

    let output = format_schema(&schema, false);
    assert!(!output.contains("Ping (Cast)"));
    assert!(output.contains("GetValue (RPC)"));
    assert!(output.contains("returns: i32"));
    assert!(output.contains("1 internal variant hidden"));
}

#[test]
fn format_schema___no_rpc_variants___shows_hint() {
    let schema = serde_json::json!({
        "variants": {
            "Timeout": {
                "fields": {"0": "u64"},
                "rpc": false
            },
            "Heartbeat": {
                "fields": {},
                "rpc": false
            }
        }
    });

    let output = format_schema(&schema, false);
    assert!(!output.contains("Timeout"));
    assert!(!output.contains("Heartbeat"));
    assert!(output.contains("no RPC variants"));
    assert!(output.contains("2 internal variants hidden"));
}

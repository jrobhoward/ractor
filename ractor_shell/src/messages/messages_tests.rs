#![allow(non_snake_case)]

use crate::messages::{parse_json_input, patterns};
use serde_json::Value;

#[test]
fn parse_json_input___valid_json_object___returns_ok() {
    let input = r#"{"Ping": ["node_a", 5]}"#;

    let result = parse_json_input(input);

    assert!(result.is_ok());
}

#[test]
fn parse_json_input___valid_json_string___returns_ok() {
    let input = r#""hello""#;

    let result = parse_json_input(input);

    assert!(result.is_ok());
}

#[test]
fn enum_message___variant_with_args___returns_json_object() {
    let msg = patterns::enum_message(
        "Ping",
        vec![Value::String("node_a".to_string()), Value::Number(5.into())],
    );

    assert!(msg.is_object());
}

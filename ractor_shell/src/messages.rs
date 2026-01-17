use anyhow::{anyhow, Result};
use serde_json::Value;

/// Message construction from JSON or string input
///
/// This module handles parsing user input into actor messages.
/// Due to Ractor's strongly-typed message system, we focus on
/// well-known message types that can be serialized.

/// Attempt to parse a JSON string into a structured value
pub fn parse_json_input(input: &str) -> Result<Value> {
    // Try to parse as JSON first
    if let Ok(value) = serde_json::from_str::<Value>(input) {
        return Ok(value);
    }

    // If not valid JSON, try to construct a simple object
    // Support formats like: Ping "node_a" 5
    // or: {"Ping": ["node_a", 5]}

    // For now, just return error if not valid JSON
    Err(anyhow!(
        "Invalid JSON. Message must be valid JSON or a simple string."
    ))
}

/// Parse a JSON value into raw bytes for network transmission
/// This uses serde_json's serialization to create a message payload
pub fn json_to_bytes(value: &Value) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|e| anyhow!("Failed to serialize JSON to bytes: {}", e))
}

/// Construct a simple text message as JSON
pub fn text_message(text: &str) -> Value {
    Value::String(text.to_string())
}

/// Helper to construct common message patterns
pub mod patterns {
    use super::*;

    /// Create a simple enum variant message
    /// Example: enum_message("Ping", vec!["node_a", "5"])
    pub fn enum_message(variant: &str, fields: Vec<Value>) -> Value {
        serde_json::json!({
            variant: fields
        })
    }

    /// Create a struct-like message
    pub fn struct_message(fields: Vec<(&str, Value)>) -> Value {
        let map: serde_json::Map<String, Value> = fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_object() {
        let input = r#"{"Ping": ["node_a", 5]}"#;
        let result = parse_json_input(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_json_string() {
        let input = r#""hello""#;
        let result = parse_json_input(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enum_message() {
        let msg = patterns::enum_message(
            "Ping",
            vec![Value::String("node_a".to_string()), Value::Number(5.into())],
        );
        assert!(msg.is_object());
    }
}

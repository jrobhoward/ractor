//! Schema Registry for Shell Introspection
//!
//! This module provides a global registry for message schemas, enabling the shell
//! to discover and display typed message schemas for actors.
//!
//! ## Usage
//!
//! Actors register their schemas in `pre_start`:
//!
//! ```ignore
//! async fn pre_start(&self, myself: ActorRef<Self::Msg>, _: ()) -> Result<Self::State, ActorProcessingErr> {
//!     if let Some(name) = myself.get_name() {
//!         ractor_shell::schema_registry::register::<MyMessage>(&name);
//!     }
//!     Ok(MyState::default())
//! }
//! ```
//!
//! And unregister in `post_stop`:
//!
//! ```ignore
//! async fn post_stop(&self, myself: ActorRef<Self::Msg>, _: &mut Self::State) -> Result<(), ActorProcessingErr> {
//!     if let Some(name) = myself.get_name() {
//!         ractor_shell::schema_registry::unregister(&name);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Note on Message Sending
//!
//! The schema registry stores schemas for display purposes. Actual message sending
//! for schema-enabled actors is handled through the IntrospectionActor protocol,
//! which has compile-time knowledge of the message types.

use ractor::SchemaProvider;
use std::collections::HashMap;
use std::sync::RwLock;

/// Global registry mapping actor names to their schema JSON strings.
static SCHEMA_REGISTRY: RwLock<Option<HashMap<String, &'static str>>> = RwLock::new(None);

/// Initialize the registry if needed.
fn ensure_initialized() {
    let mut registry = SCHEMA_REGISTRY.write().unwrap();
    if registry.is_none() {
        *registry = Some(HashMap::new());
    }
}

/// Register a schema-enabled message type for an actor.
///
/// Call this in the actor's `pre_start` method after confirming the actor has a name.
///
/// # Type Parameters
///
/// * `M` - The message type, which must implement `SchemaProvider`
///
/// # Arguments
///
/// * `actor_name` - The registered name of the actor
///
/// # Example
///
/// ```ignore
/// use ractor_shell::schema_registry;
///
/// async fn pre_start(&self, myself: ActorRef<Self::Msg>, _: ()) -> Result<Self::State, ActorProcessingErr> {
///     if let Some(name) = myself.get_name() {
///         schema_registry::register::<MyMessage>(&name);
///     }
///     Ok(MyState::default())
/// }
/// ```
pub fn register<M: SchemaProvider + 'static>(actor_name: &str) {
    ensure_initialized();
    if let Ok(mut registry) = SCHEMA_REGISTRY.write() {
        if let Some(map) = registry.as_mut() {
            map.insert(actor_name.to_string(), M::message_schema());
        }
    }
}

/// Unregister a schema when an actor stops.
///
/// Call this in the actor's `post_stop` method.
///
/// # Arguments
///
/// * `actor_name` - The registered name of the actor
pub fn unregister(actor_name: &str) {
    if let Ok(mut registry) = SCHEMA_REGISTRY.write() {
        if let Some(map) = registry.as_mut() {
            map.remove(actor_name);
        }
    }
}

/// Check if an actor has a registered schema.
///
/// # Arguments
///
/// * `actor_name` - The registered name of the actor
///
/// # Returns
///
/// `true` if the actor has a registered schema, `false` otherwise
pub fn has_schema(actor_name: &str) -> bool {
    SCHEMA_REGISTRY
        .read()
        .ok()
        .and_then(|r| r.as_ref().map(|m| m.contains_key(actor_name)))
        .unwrap_or(false)
}

/// Get the JSON schema for an actor's message type.
///
/// # Arguments
///
/// * `actor_name` - The registered name of the actor
///
/// # Returns
///
/// The JSON schema string if the actor has a registered schema, `None` otherwise
pub fn get_schema(actor_name: &str) -> Option<String> {
    SCHEMA_REGISTRY.read().ok().and_then(|r| {
        r.as_ref()
            .and_then(|m| m.get(actor_name).map(|s| s.to_string()))
    })
}

/// Get all registered actor names and their schemas.
///
/// # Returns
///
/// A vector of (actor_name, schema_json) pairs
pub fn list_schemas() -> Vec<(String, String)> {
    SCHEMA_REGISTRY
        .read()
        .ok()
        .and_then(|r| {
            r.as_ref().map(|m| {
                m.iter()
                    .map(|(name, schema)| (name.clone(), schema.to_string()))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Parse a schema JSON string and return it as a serde_json::Value.
///
/// # Arguments
///
/// * `actor_name` - The registered name of the actor
///
/// # Returns
///
/// The parsed schema as a JSON Value, or None if not found or invalid
pub fn get_schema_parsed(actor_name: &str) -> Option<serde_json::Value> {
    get_schema(actor_name).and_then(|s| serde_json::from_str(&s).ok())
}

/// Format a schema for display.
///
/// # Arguments
///
/// * `schema` - The parsed schema JSON
///
/// # Returns
///
/// A formatted string representation of the schema
pub fn format_schema(schema: &serde_json::Value) -> String {
    let mut output = String::new();

    if let Some(variants) = schema.get("variants").and_then(|v| v.as_object()) {
        for (variant_name, variant_info) in variants {
            let is_rpc = variant_info
                .get("rpc")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_rpc {
                output.push_str(&format!("  {} (RPC):\n", variant_name));
            } else {
                output.push_str(&format!("  {}:\n", variant_name));
            }

            if let Some(fields) = variant_info.get("fields").and_then(|v| v.as_object()) {
                if fields.is_empty() {
                    output.push_str("    (no fields)\n");
                } else {
                    for (field_name, field_type) in fields {
                        let type_str = field_type.as_str().unwrap_or("unknown");
                        output.push_str(&format!("    {}: {}\n", field_name, type_str));
                    }
                }
            }

            if is_rpc {
                if let Some(reply_type) = variant_info.get("reply_type").and_then(|v| v.as_str()) {
                    output.push_str(&format!("    → returns: {}\n", reply_type));
                }
            }

            output.push('\n');
        }
    }

    output
}

#[cfg(test)]
mod schema_registry_tests;

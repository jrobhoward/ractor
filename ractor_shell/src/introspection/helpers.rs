//! Helper Functions for Introspection
//!
//! This module contains helper functions for dynamic message handling and pattern matching.

use ractor::rpc::CallResult;
use ractor::{registry, ActorRef};

use crate::dynamic::{supports_dynamic_messages, CallResponse, DynamicMessage};
use crate::protocol::{DynamicCallResult, DynamicSendResult, TypedRpcResult};
use crate::tracing::{TraceEvent, TraceFilter};
use crate::DEFAULT_RPC_TIMEOUT;

/// Send a dynamic message to an actor (cast - fire and forget)
pub(crate) async fn send_dynamic_message_to_actor(
    actor_name: &str,
    json_value: serde_json::Value,
) -> DynamicSendResult {
    // Find the actor in registry
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return DynamicSendResult::ActorNotFound;
    };

    // Convert to dynamic message actor reference
    let dynamic_ref: ActorRef<DynamicMessage> = ActorRef::from(cell.clone());

    // Check if the actor supports dynamic messages
    if !supports_dynamic_messages(dynamic_ref.clone()).await {
        return DynamicSendResult::NotDynamic;
    }

    // Send the message
    match dynamic_ref.cast(DynamicMessage::Cast(json_value)) {
        Ok(()) => DynamicSendResult::Success,
        Err(e) => DynamicSendResult::SendFailed(format!("{:?}", e)),
    }
}

/// Call an actor with a dynamic message (RPC - wait for response)
pub(crate) async fn call_dynamic_message_to_actor(
    actor_name: &str,
    json_value: serde_json::Value,
) -> DynamicCallResult {
    // Find the actor in registry
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return DynamicCallResult::ActorNotFound;
    };

    // Convert to dynamic message actor reference
    let dynamic_ref: ActorRef<DynamicMessage> = ActorRef::from(cell.clone());

    // Check if the actor supports dynamic messages
    if !supports_dynamic_messages(dynamic_ref.clone()).await {
        return DynamicCallResult::NotDynamic;
    }

    // Make the RPC call
    let call_result = dynamic_ref
        .call(
            |reply| DynamicMessage::Call(json_value, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match call_result {
        Ok(CallResult::Success(response)) => match response {
            CallResponse::Success(value) => DynamicCallResult::Success(value),
            CallResponse::Error(err) => DynamicCallResult::Error(err),
        },
        Ok(CallResult::Timeout) => {
            DynamicCallResult::CallFailed(format!("Timeout after {:?}", DEFAULT_RPC_TIMEOUT))
        }
        Ok(CallResult::SenderError) => {
            DynamicCallResult::CallFailed("Sender error (actor may have stopped)".to_string())
        }
        Err(e) => DynamicCallResult::CallFailed(format!("{:?}", e)),
    }
}

/// Call a typed RPC on a schema-enabled actor
///
/// Note: Typed RPC dispatch requires compile-time knowledge of the message type.
/// Since actors define their own message types in user code (not the library),
/// this function currently only supports schema introspection (listing variants)
/// but not actual RPC calls. Use the `call` shell command with JSON messages
/// for actors that implement `DynamicMessage`.
pub(crate) async fn call_typed_rpc_on_actor(
    actor_name: &str,
    variant_name: &str,
    args: serde_json::Value,
) -> TypedRpcResult {
    // Check if the actor has a registered schema
    if !crate::schema_registry::has_schema(actor_name) {
        return TypedRpcResult::NotSchemaEnabled;
    }

    // Find the actor in registry
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return TypedRpcResult::ActorNotFound;
    };

    // Try dispatcher-based RPC first
    if let Some(dispatcher) = crate::schema_registry::get_dispatcher(actor_name) {
        match dispatcher(cell, variant_name.to_string(), args).await {
            Ok(result) => return TypedRpcResult::Success(result),
            Err(e) => return TypedRpcResult::CallFailed(e),
        }
    }

    // Fallback: no dispatcher available
    TypedRpcResult::CallFailed(format!(
        "Typed RPC for '{}::{}' not available via introspection. \
         Actor has schema but no dispatcher. Add #[ractor_shell] to enable RPC.",
        actor_name, variant_name
    ))
}

/// Check if a trace event matches a subscription pattern
pub(crate) fn matches_pattern(pattern: &str, event: &TraceEvent) -> bool {
    // Special case: "*" matches everything
    if pattern == "*" {
        return true;
    }

    // Check actor name if present
    if let Some(actor_name) = &event.actor_name {
        if wildcard_match(pattern, actor_name) {
            return true;
        }
    }

    // Check actor ID if present
    if let Some(actor_id) = &event.actor_id {
        if wildcard_match(pattern, actor_id) {
            return true;
        }
    }

    // Check target (module path) - allows patterns like "*raft*" to match "ractor_shell::raft"
    if wildcard_match(pattern, &event.target) {
        return true;
    }

    false
}

/// Simple wildcard matching (supports * and ? wildcards)
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    // Use the TraceFilter's matching logic
    let mut filter = TraceFilter::new();
    filter.add_pattern(pattern);
    filter.matches(Some(text))
}

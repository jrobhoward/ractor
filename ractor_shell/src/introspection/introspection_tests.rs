//! Tests for introspection module
#![allow(non_snake_case)]

use super::*;
use crate::dynamic::{CallResponse, DynamicMessage};
use crate::protocol::{DynamicCallResult, DynamicSendResult, TypedRpcResult};
use crate::tracing::{TraceEvent, TraceEventType};
use chrono::Local;
use ractor::{Actor, ActorProcessingErr, ActorRef};

// ============================================================================
// TraceSubscription Tests
// ============================================================================

#[test]
fn TraceSubscription_new___with_pattern___creates_empty_subscription() {
    let sub = TraceSubscription::new("test_*".to_string());

    assert_eq!(sub.pattern, "test_*");
    assert!(sub.buffer.is_empty());
    assert_eq!(sub.dropped_count, 0);
}

#[test]
fn TraceSubscription_push_event___single_event___adds_to_buffer() {
    let mut sub = TraceSubscription::new("*".to_string());
    let event = create_test_event("test_actor", "test message");

    sub.push_event(event);

    assert_eq!(sub.buffer.len(), 1);
    assert_eq!(sub.dropped_count, 0);
}

#[test]
fn TraceSubscription_push_event___exceeds_max_buffer___drops_oldest() {
    let mut sub = TraceSubscription::new("*".to_string());

    // Fill buffer to max
    for i in 0..MAX_TRACE_BUFFER_SIZE {
        let event = create_test_event("actor", &format!("message {}", i));
        sub.push_event(event);
    }
    assert_eq!(sub.buffer.len(), MAX_TRACE_BUFFER_SIZE);
    assert_eq!(sub.dropped_count, 0);

    // Add one more - should drop oldest
    let event = create_test_event("actor", "overflow message");
    sub.push_event(event);

    assert_eq!(sub.buffer.len(), MAX_TRACE_BUFFER_SIZE);
    assert_eq!(sub.dropped_count, 1);
    // First message should be "message 1" (not "message 0" which was dropped)
    assert_eq!(sub.buffer.front().unwrap().message, "message 1");
    // Last message should be our overflow message
    assert_eq!(sub.buffer.back().unwrap().message, "overflow message");
}

#[test]
fn TraceSubscription_take_events___with_events___returns_all_and_clears() {
    let mut sub = TraceSubscription::new("*".to_string());
    sub.push_event(create_test_event("actor1", "msg1"));
    sub.push_event(create_test_event("actor2", "msg2"));

    let (events, dropped) = sub.take_events();

    assert_eq!(events.len(), 2);
    assert_eq!(dropped, 0);
    assert!(sub.buffer.is_empty());
}

#[test]
fn TraceSubscription_take_events___with_dropped___returns_dropped_count_and_resets() {
    let mut sub = TraceSubscription::new("*".to_string());

    // Fill and overflow
    for i in 0..MAX_TRACE_BUFFER_SIZE + 5 {
        sub.push_event(create_test_event("actor", &format!("msg {}", i)));
    }
    assert_eq!(sub.dropped_count, 5);

    let (events, dropped) = sub.take_events();

    assert_eq!(events.len(), MAX_TRACE_BUFFER_SIZE);
    assert_eq!(dropped, 5);
    assert_eq!(sub.dropped_count, 0); // Reset after take
}

// ============================================================================
// IntrospectionArgs Tests
// ============================================================================

#[test]
fn IntrospectionArgs_from_string___converts_correctly() {
    let args: IntrospectionArgs = "my_node".to_string().into();

    assert_eq!(args.node_name, "my_node");
    assert!(args.tracing_handle.is_none());
}

// ============================================================================
// matches_pattern Tests
// ============================================================================

#[test]
fn matches_pattern___star_pattern___matches_everything() {
    let event = create_test_event("any_actor", "any message");

    assert!(matches_pattern("*", &event));
}

#[test]
fn matches_pattern___actor_name_match___returns_true() {
    let event = create_test_event("worker_1", "message");

    assert!(matches_pattern("worker_*", &event));
    assert!(matches_pattern("worker_1", &event));
}

#[test]
fn matches_pattern___actor_id_match___returns_true() {
    let mut event = create_test_event("named", "message");
    event.actor_name = None;
    event.actor_id = Some("0.42".to_string());

    assert!(matches_pattern("0.*", &event));
    assert!(matches_pattern("0.42", &event));
}

#[test]
fn matches_pattern___target_match___returns_true() {
    let mut event = create_test_event("actor", "message");
    event.target = "ractor_shell::raft::election".to_string();

    assert!(matches_pattern("*raft*", &event));
    assert!(matches_pattern("ractor_shell::raft*", &event));
}

#[test]
fn matches_pattern___no_match___returns_false() {
    let event = create_test_event("supervisor", "message");

    assert!(!matches_pattern("worker_*", &event));
    assert!(!matches_pattern("unknown", &event));
}

// ============================================================================
// wildcard_match Tests
// ============================================================================

#[test]
fn wildcard_match___exact_match___returns_true() {
    assert!(wildcard_match("hello", "hello"));
}

#[test]
fn wildcard_match___star_wildcard___matches_any_sequence() {
    assert!(wildcard_match("hello*", "hello_world"));
    assert!(wildcard_match("*world", "hello_world"));
    assert!(wildcard_match("*llo*", "hello_world"));
    assert!(wildcard_match("*", "anything"));
}

#[test]
fn wildcard_match___question_mark___matches_single_char() {
    assert!(wildcard_match("hell?", "hello"));
    assert!(wildcard_match("h?llo", "hello"));
    assert!(!wildcard_match("hell?", "hellooo"));
}

#[test]
fn wildcard_match___no_match___returns_false() {
    assert!(!wildcard_match("hello", "world"));
    assert!(!wildcard_match("hello*", "world"));
}

// ============================================================================
// extract_node_id Tests
// ============================================================================

#[test]
fn extract_node_id___standard_format___extracts_first_part() {
    assert_eq!(extract_node_id("1.42"), "1");
    assert_eq!(extract_node_id("0.1"), "0");
    assert_eq!(extract_node_id("123.456"), "123");
}

#[test]
fn extract_node_id___no_dot___returns_whole_string() {
    assert_eq!(extract_node_id("42"), "42");
}

#[test]
fn extract_node_id___empty_string___returns_empty() {
    // split('.') on "" returns iterator with one "" element, so .next() is Some("")
    assert_eq!(extract_node_id(""), "");
}

#[test]
fn extract_node_id___multiple_dots___returns_first_part() {
    assert_eq!(extract_node_id("1.2.3"), "1");
}

// ============================================================================
// Dynamic Message Actor Tests
// ============================================================================

/// Test actor that supports DynamicMessage
struct DynamicTestActor;

impl Actor for DynamicTestActor {
    type Msg = DynamicMessage;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DynamicMessage::Cast(_json) => Ok(()),
            DynamicMessage::Call(json, reply) => {
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "echo" => {
                            let _ = reply.send(CallResponse::Success(json.clone()));
                        }
                        "error" => {
                            let _ =
                                reply.send(CallResponse::Error("intentional error".to_string()));
                        }
                        _ => {
                            let _ = reply.send(CallResponse::Error(format!("unknown: {}", cmd)));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error("missing command".to_string()));
                }
                Ok(())
            }
            DynamicMessage::Ping(reply) => {
                let _ = reply.send(true);
                Ok(())
            }
        }
    }
}

/// Test actor that does NOT support DynamicMessage
struct NonDynamicTestActor;

#[derive(Debug)]
struct SimpleMessage(#[allow(dead_code)] String);
impl ractor::Message for SimpleMessage {}

impl Actor for NonDynamicTestActor {
    type Msg = SimpleMessage;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        _message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }
}

// ============================================================================
// send_dynamic_message_to_actor Tests
// ============================================================================

#[tokio::test]
async fn send_dynamic_message_to_actor___actor_not_found___returns_actor_not_found() {
    let result =
        send_dynamic_message_to_actor("nonexistent_actor_12345", serde_json::json!({})).await;

    assert!(matches!(result, DynamicSendResult::ActorNotFound));
}

#[tokio::test]
async fn send_dynamic_message_to_actor___actor_not_dynamic___returns_not_dynamic() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("non_dynamic_send_test".to_string()),
        NonDynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result =
        send_dynamic_message_to_actor("non_dynamic_send_test", serde_json::json!({})).await;

    assert!(matches!(result, DynamicSendResult::NotDynamic));
    actor_ref.stop(None);
}

#[tokio::test]
async fn send_dynamic_message_to_actor___dynamic_actor___returns_success() {
    let (actor_ref, _handle) =
        Actor::spawn(Some("dynamic_send_test".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn actor");

    let result =
        send_dynamic_message_to_actor("dynamic_send_test", serde_json::json!({"data": "test"}))
            .await;

    assert!(matches!(result, DynamicSendResult::Success));
    actor_ref.stop(None);
}

// ============================================================================
// call_dynamic_message_to_actor Tests
// ============================================================================

#[tokio::test]
async fn call_dynamic_message_to_actor___actor_not_found___returns_actor_not_found() {
    let result =
        call_dynamic_message_to_actor("nonexistent_actor_67890", serde_json::json!({})).await;

    assert!(matches!(result, DynamicCallResult::ActorNotFound));
}

#[tokio::test]
async fn call_dynamic_message_to_actor___actor_not_dynamic___returns_not_dynamic() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("non_dynamic_call_test".to_string()),
        NonDynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result =
        call_dynamic_message_to_actor("non_dynamic_call_test", serde_json::json!({})).await;

    assert!(matches!(result, DynamicCallResult::NotDynamic));
    actor_ref.stop(None);
}

#[tokio::test]
async fn call_dynamic_message_to_actor___success_response___returns_success_with_value() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("dynamic_call_success_test".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = call_dynamic_message_to_actor(
        "dynamic_call_success_test",
        serde_json::json!({"command": "echo", "data": 42}),
    )
    .await;

    match result {
        DynamicCallResult::Success(value) => {
            assert_eq!(value.get("command").unwrap(), "echo");
            assert_eq!(value.get("data").unwrap(), 42);
        }
        other => panic!("Expected Success, got {:?}", other),
    }
    actor_ref.stop(None);
}

#[tokio::test]
async fn call_dynamic_message_to_actor___error_response___returns_error() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("dynamic_call_error_test".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = call_dynamic_message_to_actor(
        "dynamic_call_error_test",
        serde_json::json!({"command": "error"}),
    )
    .await;

    match result {
        DynamicCallResult::Error(msg) => {
            assert_eq!(msg, "intentional error");
        }
        other => panic!("Expected Error, got {:?}", other),
    }
    actor_ref.stop(None);
}

// ============================================================================
// call_typed_rpc_on_actor Tests
// ============================================================================

#[tokio::test]
async fn call_typed_rpc_on_actor___no_schema___returns_not_schema_enabled() {
    // Actor without schema registration
    let (actor_ref, _handle) =
        Actor::spawn(Some("no_schema_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn actor");

    let result =
        call_typed_rpc_on_actor("no_schema_actor", "SomeVariant", serde_json::json!({})).await;

    assert!(matches!(result, TypedRpcResult::NotSchemaEnabled));
    actor_ref.stop(None);
}

// Note: Testing "actor_not_found with schema" requires creating a SchemaProvider implementation,
// which is complex. The NotSchemaEnabled case covers the common path.
// The ActorNotFound case is tested indirectly through IntrospectionActor tests.

// ============================================================================
// build_cluster_topology Tests
// ============================================================================

#[tokio::test]
async fn build_cluster_topology___local_node___includes_local_node_info() {
    // Spawn a test actor so there's something in the registry
    let (actor_ref, _handle) = Actor::spawn(
        Some("topology_test_actor".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let topology = build_cluster_topology("test_node");

    // Should have at least one node (local)
    assert!(!topology.nodes.is_empty());

    // Local node should have our node name
    let local_node = topology.nodes.iter().find(|n| n.is_local);
    assert!(local_node.is_some());
    assert_eq!(local_node.unwrap().name, "test_node");

    actor_ref.stop(None);
}

// ============================================================================
// build_supervision_tree_roots Tests
// ============================================================================

#[tokio::test]
async fn build_supervision_tree_roots___with_registered_actor___includes_actor() {
    let (actor_ref, _handle) =
        Actor::spawn(Some("tree_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn actor");

    let roots = build_supervision_tree_roots();

    // Should find our actor (it has no supervisor, so it's a root)
    let found = roots
        .iter()
        .any(|node| node.actor.name.as_deref() == Some("tree_test_actor"));
    assert!(
        found,
        "Should find tree_test_actor in supervision tree roots"
    );

    actor_ref.stop(None);
}

// ============================================================================
// IntrospectionActor Integration Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___spawn___joins_introspection_group() {
    let args = IntrospectionArgs {
        node_name: "test_introspection_node".to_string(),
        tracing_handle: None,
    };

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Check that it joined the introspection group
    let members = pg::get_members(&INTROSPECTION_GROUP.to_string());
    let found = members
        .iter()
        .any(|cell| cell.get_name().as_deref() == Some("test_introspection"));
    assert!(found, "IntrospectionActor should join INTROSPECTION_GROUP");

    actor_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___ping___returns_pong_with_node_name() {
    let args = IntrospectionArgs {
        node_name: "ping_test_node".to_string(),
        tracing_handle: None,
    };

    let (actor_ref, _handle) = Actor::spawn(
        Some("ping_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = actor_ref
        .call(ShellProtocolMessage::Ping, Some(DEFAULT_RPC_TIMEOUT))
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(response)) => {
            assert!(response.contains("ping_test_node"));
        }
        other => panic!("Expected successful ping, got {:?}", other),
    }

    actor_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___list_registered_actors___returns_actor_list() {
    // Spawn a test actor
    let (test_actor_ref, _test_handle) =
        Actor::spawn(Some("list_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "list_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("list_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            ShellProtocolMessage::ListRegisteredActors,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(actors)) => {
            let found = actors
                .iter()
                .any(|a| a.name.as_deref() == Some("list_test_actor"));
            assert!(found, "Should find list_test_actor in registered actors");
        }
        other => panic!("Expected actor list, got {:?}", other),
    }

    test_actor_ref.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_actor_info___returns_info_for_existing_actor() {
    let (test_actor_ref, _test_handle) =
        Actor::spawn(Some("info_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "info_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("info_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::GetActorInfo("info_test_actor".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(Some(info))) => {
            assert_eq!(info.name.as_deref(), Some("info_test_actor"));
            assert_eq!(info.status, "Running");
        }
        other => panic!("Expected actor info, got {:?}", other),
    }

    test_actor_ref.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_actor_info___returns_none_for_nonexistent() {
    let args = IntrospectionArgs {
        node_name: "none_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("none_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::GetActorInfo("nonexistent_xyz_123".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(None)) => {
            // Expected
        }
        other => panic!("Expected None, got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_event(actor_name: &str, message: &str) -> TraceEvent {
    TraceEvent {
        timestamp: Local::now(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some(actor_name.to_string()),
        event_type: TraceEventType::Event,
        level: tracing::Level::INFO,
        target: "test::module".to_string(),
        message: message.to_string(),
        fields: vec![],
    }
}

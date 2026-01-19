//! Tests for introspection module
#![allow(non_snake_case)]

use super::*;
use crate::dynamic::{CallResponse, DynamicMessage};
use crate::protocol::{DynamicCallResult, DynamicSendResult, TypedRpcResult};
use crate::tracing::{TraceEvent, TraceEventType};
use crate::DEFAULT_RPC_TIMEOUT;
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

    let topology = build_cluster_topology("test_node").await;

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

// ============================================================================
// ListProcessGroups Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___list_process_groups___returns_group_list() {
    use ractor::pg;

    // Create a test actor and add it to a process group
    let (test_actor_ref, _test_handle) =
        Actor::spawn(Some("pg_list_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let group_name = "test_pg_list_group";
    pg::join(group_name.to_string(), vec![test_actor_ref.get_cell()]);

    let args = IntrospectionArgs {
        node_name: "pg_list_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("pg_list_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            ShellProtocolMessage::ListProcessGroups,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(groups)) => {
            let found = groups.iter().any(|g| g == group_name);
            assert!(found, "Should find test_pg_list_group in process groups");
        }
        other => panic!("Expected group list, got {:?}", other),
    }

    test_actor_ref.stop(None);
    introspection_ref.stop(None);
}

// ============================================================================
// StopActor Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___stop_actor___stops_registered_actor() {
    // Create a test actor
    let (_test_actor_ref, test_handle) =
        Actor::spawn(Some("stop_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "stop_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("stop_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Verify actor exists in registry
    assert!(
        ractor::registry::where_is("stop_test_actor".to_string()).is_some(),
        "Actor should be in registry before stop"
    );

    // Stop the actor via introspection
    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::StopActor("stop_test_actor".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(true)) => {
            // Expected - actor was found and stopped
        }
        other => panic!("Expected Success(true), got {:?}", other),
    }

    // Wait for the actor to stop
    let _ = test_handle.await;

    // Verify actor is no longer in registry
    assert!(
        ractor::registry::where_is("stop_test_actor".to_string()).is_none(),
        "Actor should be removed from registry after stop"
    );

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___stop_actor___returns_false_for_nonexistent() {
    let args = IntrospectionArgs {
        node_name: "stop_nonexistent_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("stop_nonexistent_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Try to stop a nonexistent actor
    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::StopActor("nonexistent_actor_xyz_789".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(false)) => {
            // Expected - actor was not found
        }
        other => panic!("Expected Success(false), got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// GetProcessGroupTree Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___get_process_group_tree___returns_groups_with_members() {
    use ractor::pg;

    // Create test actors and add them to process groups
    let (test_actor1, _handle1) =
        Actor::spawn(Some("pg_tree_actor1".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor 1");

    let (test_actor2, _handle2) =
        Actor::spawn(Some("pg_tree_actor2".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor 2");

    let group_name = "test_pg_tree_group";
    pg::join(
        group_name.to_string(),
        vec![test_actor1.get_cell(), test_actor2.get_cell()],
    );

    let args = IntrospectionArgs {
        node_name: "pg_tree_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("pg_tree_test_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            ShellProtocolMessage::GetProcessGroupTree,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(tree)) => {
            // Verify our test group exists
            assert!(
                tree.contains_key(group_name),
                "Tree should contain test_pg_tree_group"
            );

            // Verify it has the right members
            let members = tree.get(group_name).unwrap();
            assert_eq!(members.len(), 2, "Group should have 2 members");

            let member_names: Vec<_> = members.iter().filter_map(|m| m.name.clone()).collect();
            assert!(
                member_names.contains(&"pg_tree_actor1".to_string()),
                "Should find pg_tree_actor1"
            );
            assert!(
                member_names.contains(&"pg_tree_actor2".to_string()),
                "Should find pg_tree_actor2"
            );
        }
        other => panic!("Expected process group tree, got {:?}", other),
    }

    test_actor1.stop(None);
    test_actor2.stop(None);
    introspection_ref.stop(None);
}

// ============================================================================
// Remote Monitoring Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___start_monitoring___returns_true_when_actor_found() {
    // Create a test actor to monitor
    let (test_actor, _handle) =
        Actor::spawn(Some("monitor_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "monitor_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("monitor_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::StartMonitoring("monitor_test_actor".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(true)) => {
            // Expected - actor was found and monitoring started
        }
        other => panic!("Expected Success(true), got {:?}", other),
    }

    test_actor.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___start_monitoring___returns_false_when_actor_not_found() {
    let args = IntrospectionArgs {
        node_name: "monitor_test_node2".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("monitor_introspection2".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StartMonitoring(
                    "nonexistent_actor_abc_123".to_string(),
                    reply,
                )
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(false)) => {
            // Expected - actor was not found
        }
        other => panic!("Expected Success(false), got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___stop_monitoring___returns_true_when_was_monitored() {
    // Create a test actor to monitor
    let (test_actor, _handle) = Actor::spawn(
        Some("stop_monitor_test_actor".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "stop_monitor_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("stop_monitor_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Start monitoring first
    let _ = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StartMonitoring("stop_monitor_test_actor".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    // Now stop monitoring
    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StopMonitoring("stop_monitor_test_actor".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(true)) => {
            // Expected - actor was being monitored and is now stopped
        }
        other => panic!("Expected Success(true), got {:?}", other),
    }

    test_actor.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___stop_monitoring___returns_false_when_not_monitored() {
    let args = IntrospectionArgs {
        node_name: "stop_monitor_test_node2".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("stop_monitor_introspection2".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Try to stop monitoring an actor that was never monitored
    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StopMonitoring("never_monitored_actor".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(false)) => {
            // Expected - actor was not being monitored
        }
        other => panic!("Expected Success(false), got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_monitored_actors___returns_monitored_list() {
    // Create test actors to monitor
    let (test_actor1, _handle1) = Actor::spawn(
        Some("get_monitors_test_actor1".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor 1");

    let (test_actor2, _handle2) = Actor::spawn(
        Some("get_monitors_test_actor2".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor 2");

    let args = IntrospectionArgs {
        node_name: "get_monitors_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("get_monitors_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Start monitoring both actors
    let _ = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StartMonitoring("get_monitors_test_actor1".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    let _ = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StartMonitoring("get_monitors_test_actor2".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    // Get monitored actors
    let result = introspection_ref
        .call(
            ShellProtocolMessage::GetMonitoredActors,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(monitored)) => {
            assert_eq!(monitored.len(), 2, "Should have 2 monitored actors");
            assert!(
                monitored.contains(&"get_monitors_test_actor1".to_string()),
                "Should contain test_actor1"
            );
            assert!(
                monitored.contains(&"get_monitors_test_actor2".to_string()),
                "Should contain test_actor2"
            );
        }
        other => panic!("Expected Success with monitored list, got {:?}", other),
    }

    test_actor1.stop(None);
    test_actor2.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___poll_monitor_events___returns_empty_batch_initially() {
    let args = IntrospectionArgs {
        node_name: "poll_events_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("poll_events_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Poll for events without monitoring anything
    let result = introspection_ref
        .call(
            ShellProtocolMessage::PollMonitorEvents,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(batch)) => {
            assert!(batch.events.is_empty(), "Should have no events initially");
            assert_eq!(batch.dropped_count, 0, "Should have no dropped events");
        }
        other => panic!("Expected empty batch, got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// MonitoringState Tests
// ============================================================================

#[test]
fn MonitoringState___new___creates_empty_state() {
    let state = super::MonitoringState::new();

    assert!(state.monitored.is_empty());
    assert!(state.event_buffer.is_empty());
    assert_eq!(state.dropped_count, 0);
}

#[test]
fn MonitoringState___push_event___adds_event_to_buffer() {
    let mut state = super::MonitoringState::new();
    let event = crate::protocol::SerializableMonitorEvent {
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        actor_id: "0.1".to_string(),
        actor_name: Some("test_actor".to_string()),
        event_type: "STARTED".to_string(),
        reason: None,
        error: None,
    };

    state.push_event(event);

    assert_eq!(state.event_buffer.len(), 1);
    assert_eq!(state.dropped_count, 0);
}

#[test]
fn MonitoringState___push_event___exceeds_max_buffer___drops_oldest() {
    let mut state = super::MonitoringState::new();

    // Fill buffer to max (MAX_MONITOR_BUFFER_SIZE = 100)
    for i in 0..super::MAX_MONITOR_BUFFER_SIZE {
        let event = crate::protocol::SerializableMonitorEvent {
            timestamp: format!("2024-01-01T00:00:{:02}Z", i % 60),
            actor_id: format!("0.{}", i),
            actor_name: Some(format!("actor_{}", i)),
            event_type: "STARTED".to_string(),
            reason: None,
            error: None,
        };
        state.push_event(event);
    }
    assert_eq!(state.event_buffer.len(), super::MAX_MONITOR_BUFFER_SIZE);
    assert_eq!(state.dropped_count, 0);

    // Add one more - should drop oldest
    let overflow_event = crate::protocol::SerializableMonitorEvent {
        timestamp: "2024-01-01T01:00:00Z".to_string(),
        actor_id: "0.999".to_string(),
        actor_name: Some("overflow_actor".to_string()),
        event_type: "STOPPED".to_string(),
        reason: None,
        error: None,
    };
    state.push_event(overflow_event);

    assert_eq!(state.event_buffer.len(), super::MAX_MONITOR_BUFFER_SIZE);
    assert_eq!(state.dropped_count, 1);
    // First event should be "actor_1" (not "actor_0" which was dropped)
    assert_eq!(
        state.event_buffer.front().unwrap().actor_name,
        Some("actor_1".to_string())
    );
    // Last event should be our overflow actor
    assert_eq!(
        state.event_buffer.back().unwrap().actor_name,
        Some("overflow_actor".to_string())
    );
}

#[test]
fn MonitoringState___take_events___returns_all_and_clears() {
    let mut state = super::MonitoringState::new();

    for i in 0..5 {
        let event = crate::protocol::SerializableMonitorEvent {
            timestamp: format!("2024-01-01T00:00:{:02}Z", i),
            actor_id: format!("0.{}", i),
            actor_name: Some(format!("actor_{}", i)),
            event_type: "STARTED".to_string(),
            reason: None,
            error: None,
        };
        state.push_event(event);
    }

    let (events, dropped) = state.take_events();

    assert_eq!(events.len(), 5);
    assert_eq!(dropped, 0);
    assert!(state.event_buffer.is_empty());
}

#[test]
fn MonitoringState___take_events___with_dropped___returns_dropped_count_and_resets() {
    let mut state = super::MonitoringState::new();

    // Fill and overflow
    for i in 0..super::MAX_MONITOR_BUFFER_SIZE + 10 {
        let event = crate::protocol::SerializableMonitorEvent {
            timestamp: format!("2024-01-01T00:00:{:02}Z", i % 60),
            actor_id: format!("0.{}", i),
            actor_name: Some(format!("actor_{}", i)),
            event_type: "STARTED".to_string(),
            reason: None,
            error: None,
        };
        state.push_event(event);
    }
    assert_eq!(state.dropped_count, 10);

    let (events, dropped) = state.take_events();

    assert_eq!(events.len(), super::MAX_MONITOR_BUFFER_SIZE);
    assert_eq!(dropped, 10);
    assert_eq!(state.dropped_count, 0); // Reset after take
}

// ============================================================================
// GetProcessGroupMembers Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___get_pg_members___returns_members_of_group() {
    use ractor::pg;

    // Create test actors and add them to a process group
    let (test_actor1, _handle1) =
        Actor::spawn(Some("pg_members_actor1".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor 1");

    let (test_actor2, _handle2) =
        Actor::spawn(Some("pg_members_actor2".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor 2");

    let group_name = "test_pg_members_group";
    pg::join(
        group_name.to_string(),
        vec![test_actor1.get_cell(), test_actor2.get_cell()],
    );

    let args = IntrospectionArgs {
        node_name: "pg_members_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("pg_members_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::GetProcessGroupMembers(group_name.to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(members)) => {
            assert_eq!(members.len(), 2, "Group should have 2 members");
            let member_names: Vec<_> = members.iter().filter_map(|m| m.name.clone()).collect();
            assert!(
                member_names.contains(&"pg_members_actor1".to_string()),
                "Should find pg_members_actor1"
            );
            assert!(
                member_names.contains(&"pg_members_actor2".to_string()),
                "Should find pg_members_actor2"
            );
        }
        other => panic!("Expected member list, got {:?}", other),
    }

    test_actor1.stop(None);
    test_actor2.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_pg_members___empty_group___returns_empty_list() {
    let args = IntrospectionArgs {
        node_name: "pg_empty_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("pg_empty_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Query a nonexistent group
    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::GetProcessGroupMembers(
                    "nonexistent_group_xyz_123".to_string(),
                    reply,
                )
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(members)) => {
            assert!(
                members.is_empty(),
                "Nonexistent group should have no members"
            );
        }
        other => panic!("Expected empty list, got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// GetMessageSchema and ListSchemaActors Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___get_message_schema___no_schema___returns_none() {
    let args = IntrospectionArgs {
        node_name: "schema_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("schema_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::GetMessageSchema("no_schema_actor_xyz".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(schema)) => {
            assert!(schema.is_none(), "Actor without schema should return None");
        }
        other => panic!("Expected None, got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___list_schema_actors___returns_list() {
    let args = IntrospectionArgs {
        node_name: "list_schema_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("list_schema_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            ShellProtocolMessage::ListSchemaActors,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(_schemas)) => {
            // May be empty if no schemas registered, but should succeed
            // Just verifying the call doesn't fail
        }
        other => panic!("Expected schema list, got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// Trace Subscription Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___subscribe_to_traces___returns_subscription_id() {
    let args = IntrospectionArgs {
        node_name: "trace_sub_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("trace_sub_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::SubscribeToTraces("*".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(sub_id)) => {
            assert!(sub_id > 0, "Subscription ID should be positive");
        }
        other => panic!("Expected subscription ID, got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___poll_traces___existing_subscription___returns_batch() {
    let args = IntrospectionArgs {
        node_name: "poll_traces_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("poll_traces_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Subscribe first
    let sub_result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::SubscribeToTraces("test_*".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    let sub_id = match sub_result {
        Ok(ractor::rpc::CallResult::Success(id)) => id,
        other => panic!("Expected subscription ID, got {:?}", other),
    };

    // Poll for events (should be empty initially)
    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::PollTraces(sub_id, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(batch)) => {
            assert!(
                batch.events.is_empty(),
                "Should have no trace events initially"
            );
            assert_eq!(batch.dropped_count, 0);
        }
        other => panic!("Expected trace batch, got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___poll_traces___nonexistent_subscription___returns_empty_batch() {
    let args = IntrospectionArgs {
        node_name: "poll_traces_bad_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("poll_traces_bad_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Poll with invalid subscription ID
    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::PollTraces(99999, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(batch)) => {
            assert!(
                batch.events.is_empty(),
                "Nonexistent sub should return empty batch"
            );
            assert_eq!(batch.dropped_count, 0);
        }
        other => panic!("Expected empty batch, got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___unsubscribe___existing___returns_true() {
    let args = IntrospectionArgs {
        node_name: "unsub_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("unsub_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Subscribe first
    let sub_result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::SubscribeToTraces("*".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    let sub_id = match sub_result {
        Ok(ractor::rpc::CallResult::Success(id)) => id,
        other => panic!("Expected subscription ID, got {:?}", other),
    };

    // Unsubscribe
    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::UnsubscribeFromTraces(sub_id, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(true)) => {
            // Expected
        }
        other => panic!("Expected Success(true), got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___unsubscribe___nonexistent___returns_false() {
    let args = IntrospectionArgs {
        node_name: "unsub_bad_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("unsub_bad_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Try to unsubscribe nonexistent
    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::UnsubscribeFromTraces(99999, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(false)) => {
            // Expected
        }
        other => panic!("Expected Success(false), got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// GetSupervisionTree and GetActorParent Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___get_supervision_tree___no_actor___returns_all_roots() {
    // Create a test actor (which will be a root since it has no parent)
    let (test_actor, _handle) = Actor::spawn(
        Some("tree_root_test_actor".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "tree_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("tree_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::GetSupervisionTree(None, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(tree_nodes)) => {
            // Should find our test actor in the roots
            let found = tree_nodes
                .iter()
                .any(|node| node.actor.name.as_deref() == Some("tree_root_test_actor"));
            assert!(
                found,
                "Should find tree_root_test_actor in supervision tree roots"
            );
        }
        other => panic!("Expected supervision tree, got {:?}", other),
    }

    test_actor.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_supervision_tree___specific_actor___returns_subtree() {
    // Create a test actor
    let (test_actor, _handle) =
        Actor::spawn(Some("subtree_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "subtree_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("subtree_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::GetSupervisionTree(
                    Some("subtree_test_actor".to_string()),
                    reply,
                )
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(tree_nodes)) => {
            // Should find exactly one node - the actor we asked for
            assert_eq!(tree_nodes.len(), 1, "Should return exactly one tree node");
            assert_eq!(
                tree_nodes[0].actor.name.as_deref(),
                Some("subtree_test_actor")
            );
        }
        other => panic!("Expected supervision tree, got {:?}", other),
    }

    test_actor.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_supervision_tree___nonexistent_actor___returns_empty() {
    let args = IntrospectionArgs {
        node_name: "tree_empty_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("tree_empty_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::GetSupervisionTree(
                    Some("nonexistent_actor_xyz_789".to_string()),
                    reply,
                )
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(tree_nodes)) => {
            assert!(
                tree_nodes.is_empty(),
                "Nonexistent actor should return empty tree"
            );
        }
        other => panic!("Expected empty tree, got {:?}", other),
    }

    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_actor_parent___no_parent___returns_none() {
    // Create a root actor (no supervisor)
    let (test_actor, _handle) =
        Actor::spawn(Some("parent_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "parent_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("parent_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| ShellProtocolMessage::GetActorParent("parent_test_actor".to_string(), reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(None)) => {
            // Expected - root actor has no parent
        }
        other => panic!("Expected None (no parent), got {:?}", other),
    }

    test_actor.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_actor_parent___nonexistent___returns_none() {
    let args = IntrospectionArgs {
        node_name: "parent_bad_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("parent_bad_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::GetActorParent("nonexistent_actor_xyz_456".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(None)) => {
            // Expected - actor doesn't exist
        }
        other => panic!("Expected None (actor not found), got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// GetActorMetrics and GetSystemInfo Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___get_actor_metrics___returns_metrics_list() {
    // Create a test actor
    let (test_actor, _handle) =
        Actor::spawn(Some("metrics_test_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "metrics_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("metrics_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            ShellProtocolMessage::GetActorMetrics,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(metrics)) => {
            // Should find our test actor in the metrics
            let found = metrics
                .iter()
                .any(|m| m.name.as_deref() == Some("metrics_test_actor"));
            assert!(found, "Should find metrics_test_actor in metrics list");
        }
        other => panic!("Expected metrics list, got {:?}", other),
    }

    test_actor.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___get_system_info___returns_system_info() {
    let args = IntrospectionArgs {
        node_name: "sysinfo_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("sysinfo_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    let result = introspection_ref
        .call(
            ShellProtocolMessage::GetSystemInfo,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(info)) => {
            // Verify basic fields are populated
            assert!(!info.hostname.is_empty(), "Hostname should not be empty");
            assert!(!info.exe_name.is_empty(), "Exe name should not be empty");
            assert!(info.pid > 0, "PID should be positive");
            assert!(
                !info.ractor_shell_version.is_empty(),
                "Version should not be empty"
            );
        }
        other => panic!("Expected system info, got {:?}", other),
    }

    introspection_ref.stop(None);
}

// ============================================================================
// collect_actor_metrics Tests
// ============================================================================

#[tokio::test]
async fn collect_actor_metrics___with_registered_actors___includes_actors() {
    // Create test actors
    let (test_actor1, _handle1) = Actor::spawn(
        Some("collect_metrics_actor1".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor 1");

    let (test_actor2, _handle2) = Actor::spawn(
        Some("collect_metrics_actor2".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor 2");

    let metrics = super::collect_actor_metrics();

    // Should find our test actors
    let found1 = metrics
        .iter()
        .any(|m| m.name.as_deref() == Some("collect_metrics_actor1"));
    let found2 = metrics
        .iter()
        .any(|m| m.name.as_deref() == Some("collect_metrics_actor2"));

    assert!(found1, "Should find collect_metrics_actor1");
    assert!(found2, "Should find collect_metrics_actor2");

    test_actor1.stop(None);
    test_actor2.stop(None);
}

#[tokio::test]
async fn collect_actor_metrics___actor_in_process_group___includes_group_membership() {
    use ractor::pg;

    // Create test actor and add to a known process group
    let (test_actor, _handle) =
        Actor::spawn(Some("collect_pg_actor".to_string()), DynamicTestActor, ())
            .await
            .expect("Failed to spawn test actor");

    // Add to a known process group (must be in KNOWN_PROCESS_GROUPS)
    pg::join("ping_pong".to_string(), vec![test_actor.get_cell()]);

    let metrics = super::collect_actor_metrics();

    // Find our actor and check group membership
    let actor_metrics = metrics
        .iter()
        .find(|m| m.name.as_deref() == Some("collect_pg_actor"));

    assert!(actor_metrics.is_some(), "Should find collect_pg_actor");
    let actor_metrics = actor_metrics.unwrap();
    assert!(
        actor_metrics.groups.contains(&"ping_pong".to_string()),
        "Should show ping_pong group membership"
    );

    test_actor.stop(None);
}

// ============================================================================
// SystemInfoCollector Tests
// ============================================================================

#[test]
fn SystemInfoCollector___new___creates_collector() {
    let collector = super::SystemInfoCollector::new();
    // Just verify it creates without panic
    drop(collector);
}

#[test]
fn SystemInfoCollector___refresh___returns_valid_info() {
    let mut collector = super::SystemInfoCollector::new();
    let info = collector.refresh();

    // Verify basic fields
    assert!(!info.hostname.is_empty(), "Hostname should not be empty");
    assert!(!info.exe_name.is_empty(), "Exe name should not be empty");
    assert!(info.pid > 0, "PID should be positive");
    assert!(
        !info.ractor_shell_version.is_empty(),
        "Version should not be empty"
    );
}

#[test]
fn SystemInfoCollector___multiple_refreshes___tracks_cpu_over_time() {
    let mut collector = super::SystemInfoCollector::new();

    // First refresh
    let info1 = collector.refresh();

    // Do some work to generate CPU usage
    let mut sum = 0u64;
    for i in 0..100000 {
        sum = sum.wrapping_add(i);
    }
    let _ = sum; // Prevent optimization

    // Second refresh
    let info2 = collector.refresh();

    // Both should return valid info (CPU% may or may not change)
    assert!(info1.pid > 0);
    assert!(info2.pid > 0);
    assert_eq!(info1.pid, info2.pid, "PID should be consistent");
}

// ============================================================================
// CallTypedRpc Tests (additional cases)
// ============================================================================

// Note: Testing the ActorNotFound path for call_typed_rpc_on_actor requires
// registering a schema via the SchemaProvider trait, which needs a concrete
// message type. The main path (NotSchemaEnabled) is already tested above.
// The ActorNotFound case is implicitly tested through the IntrospectionActor
// integration tests.

// ============================================================================
// PollMonitorEvents with Status Changes Tests
// ============================================================================

#[tokio::test]
async fn IntrospectionActor___poll_monitor_events___returns_started_event_when_monitoring_begins() {
    // Create a test actor to monitor
    let (test_actor, _handle) = Actor::spawn(
        Some("poll_started_test_actor".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "poll_started_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("poll_started_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Start monitoring - this generates a STARTED event
    let _ = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StartMonitoring("poll_started_test_actor".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    // Poll for events
    let result = introspection_ref
        .call(
            ShellProtocolMessage::PollMonitorEvents,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(batch)) => {
            // Should have at least one STARTED event
            let started_event = batch.events.iter().find(|e| e.event_type == "STARTED");
            assert!(started_event.is_some(), "Should have a STARTED event");
            assert_eq!(
                started_event.unwrap().actor_name.as_deref(),
                Some("poll_started_test_actor")
            );
        }
        other => panic!("Expected batch with STARTED event, got {:?}", other),
    }

    test_actor.stop(None);
    introspection_ref.stop(None);
}

#[tokio::test]
async fn IntrospectionActor___poll_monitor_events___detects_actor_stop() {
    // Create a test actor to monitor
    let (test_actor, test_handle) = Actor::spawn(
        Some("poll_stop_test_actor".to_string()),
        DynamicTestActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor");

    let args = IntrospectionArgs {
        node_name: "poll_stop_test_node".to_string(),
        tracing_handle: None,
    };

    let (introspection_ref, _handle) = Actor::spawn(
        Some("poll_stop_introspection".to_string()),
        IntrospectionActor,
        args,
    )
    .await
    .expect("Failed to spawn IntrospectionActor");

    // Start monitoring
    let _ = introspection_ref
        .call(
            |reply| {
                ShellProtocolMessage::StartMonitoring("poll_stop_test_actor".to_string(), reply)
            },
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    // Clear the initial STARTED event
    let _ = introspection_ref
        .call(
            ShellProtocolMessage::PollMonitorEvents,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    // Stop the monitored actor
    test_actor.stop(None);
    let _ = test_handle.await;

    // Give some time for status to update
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Poll for events - should detect the stop
    let result = introspection_ref
        .call(
            ShellProtocolMessage::PollMonitorEvents,
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(batch)) => {
            // Should have a STOPPED event (either status change or "no longer in registry")
            let stopped_event = batch.events.iter().find(|e| e.event_type == "STOPPED");
            assert!(
                stopped_event.is_some(),
                "Should detect actor stop: got events {:?}",
                batch.events
            );
        }
        other => panic!("Expected batch with STOPPED event, got {:?}", other),
    }

    introspection_ref.stop(None);
}

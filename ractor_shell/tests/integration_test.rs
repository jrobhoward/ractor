//! Integration tests for ractor_shell
//!
//! These tests verify the shell works correctly with live actors.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use ractor_shell::dynamic::{CallResponse, DynamicMessage};
use ractor_shell::ShellState;
use serde_json::json;

// ==================== Test Actors ====================

/// Simple test actor for integration tests
struct TestActor {
    name: String,
}

impl Actor for TestActor {
    type Msg = DynamicMessage;
    type State = i32;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(0)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DynamicMessage::Cast(value) => {
                if let Some(cmd) = value.get("command").and_then(|v| v.as_str()) {
                    if cmd == "increment" {
                        *state += 1;
                    }
                }
                Ok(())
            }
            DynamicMessage::Call(value, reply) => {
                if let Some(cmd) = value.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "get_state" => {
                            let _ = reply.send(CallResponse::Success(
                                json!({"state": *state, "name": self.name}),
                            ));
                        }
                        "ping" => {
                            let _ = reply.send(CallResponse::Success(json!({"pong": true})));
                        }
                        _ => {
                            let _ = reply
                                .send(CallResponse::Error(format!("Unknown command: {}", cmd)));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error("No command field".to_string()));
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

// ==================== Integration Tests ====================

// Note: ShellState tests are combined into one test to avoid registry conflicts
// since ShellState::new() spawns a named "shell_monitor" actor
#[tokio::test]
async fn test_shell_state_and_prompt() {
    let state = ShellState::new().await;
    assert!(state.is_ok());

    let state = state.unwrap();

    // Test initial state
    assert!(!state.should_exit);
    assert!(state.current_node.is_none());
    assert!(state.connected_nodes.is_empty());

    // Test prompt building
    let prompt = state.build_prompt();
    // Prompt should contain "ractor" and "@local"
    // Note: colored output may include ANSI codes
    assert!(prompt.contains("ractor") || prompt.contains("\x1b"));
}

#[tokio::test]
async fn test_spawn_and_query_actor() {
    // Spawn a test actor with a name
    let (actor_ref, _handle) = Actor::spawn(
        Some("test_integration_actor".to_string()),
        TestActor {
            name: "test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    // Verify actor is in registry
    let registered = ractor::registry::registered();
    assert!(registered.contains(&"test_integration_actor".to_string()));

    // Clean up
    actor_ref.stop(None);
}

#[tokio::test]
async fn test_dynamic_message_ping() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("test_ping_actor".to_string()),
        TestActor {
            name: "pinger".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    // Test ping functionality
    let supports = ractor_shell::dynamic::supports_dynamic_messages(actor_ref.clone()).await;
    assert!(supports);

    // Clean up
    actor_ref.stop(None);
}

#[tokio::test]
async fn test_process_group_membership() {
    // Spawn actors and add them to a process group
    let (actor1, _) = Actor::spawn(
        Some("pg_test_actor_1".to_string()),
        TestActor {
            name: "pg1".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor 1");

    let (actor2, _) = Actor::spawn(
        Some("pg_test_actor_2".to_string()),
        TestActor {
            name: "pg2".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor 2");

    // Join process group
    ractor::pg::join(
        "test_integration_group".to_string(),
        vec![actor1.get_cell(), actor2.get_cell()],
    );

    // Verify membership
    let members = ractor::pg::get_members(&"test_integration_group".to_string());
    assert_eq!(members.len(), 2);

    // Clean up
    actor1.stop(None);
    actor2.stop(None);
}

#[tokio::test]
async fn test_call_response_success() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("test_call_actor".to_string()),
        TestActor {
            name: "caller".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    // Send a call message
    use ractor::rpc::CallResult;
    let result = actor_ref
        .call(
            |reply| DynamicMessage::Call(json!({"command": "ping"}), reply),
            Some(std::time::Duration::from_secs(1)),
        )
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        CallResult::Success(response) => match response {
            CallResponse::Success(value) => {
                assert_eq!(value.get("pong"), Some(&json!(true)));
            }
            CallResponse::Error(e) => panic!("Unexpected error: {}", e),
        },
        _ => panic!("Expected success result"),
    }

    // Clean up
    actor_ref.stop(None);
}

#[tokio::test]
async fn test_call_response_error() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("test_error_actor".to_string()),
        TestActor {
            name: "error_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    // Send a call with unknown command
    use ractor::rpc::CallResult;
    let result = actor_ref
        .call(
            |reply| DynamicMessage::Call(json!({"command": "unknown"}), reply),
            Some(std::time::Duration::from_secs(1)),
        )
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        CallResult::Success(response) => match response {
            CallResponse::Error(msg) => {
                assert!(msg.contains("Unknown command"));
            }
            CallResponse::Success(_) => panic!("Expected error response"),
        },
        _ => panic!("Expected success result"),
    }

    // Clean up
    actor_ref.stop(None);
}

#[tokio::test]
async fn test_cast_message() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("test_cast_actor".to_string()),
        TestActor {
            name: "caster".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    // Send cast message to increment state
    actor_ref
        .cast(DynamicMessage::Cast(json!({"command": "increment"})))
        .expect("Failed to cast");

    // Give actor time to process
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify state changed via call
    use ractor::rpc::CallResult;
    let result = actor_ref
        .call(
            |reply| DynamicMessage::Call(json!({"command": "get_state"}), reply),
            Some(std::time::Duration::from_secs(1)),
        )
        .await;

    match result.unwrap() {
        CallResult::Success(response) => match response {
            CallResponse::Success(value) => {
                assert_eq!(value.get("state"), Some(&json!(1)));
            }
            CallResponse::Error(e) => panic!("Unexpected error: {}", e),
        },
        _ => panic!("Expected success result"),
    }

    // Clean up
    actor_ref.stop(None);
}

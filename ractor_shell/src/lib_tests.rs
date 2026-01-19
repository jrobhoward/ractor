#![allow(non_snake_case)]

use crate::should_display_level;
use crate::tracing::MinLevel;
use crate::ShellCommand;

// ==================== should_display_level Tests ====================

#[test]
fn should_display_level___trace_event_with_trace_min___returns_true() {
    assert!(should_display_level(&MinLevel::Trace, &MinLevel::Trace));
}

#[test]
fn should_display_level___debug_event_with_info_min___returns_false() {
    assert!(!should_display_level(&MinLevel::Debug, &MinLevel::Info));
}

#[test]
fn should_display_level___info_event_with_info_min___returns_true() {
    assert!(should_display_level(&MinLevel::Info, &MinLevel::Info));
}

#[test]
fn should_display_level___warn_event_with_info_min___returns_true() {
    assert!(should_display_level(&MinLevel::Warn, &MinLevel::Info));
}

#[test]
fn should_display_level___error_event_with_error_min___returns_true() {
    assert!(should_display_level(&MinLevel::Error, &MinLevel::Error));
}

#[test]
fn should_display_level___info_event_with_error_min___returns_false() {
    assert!(!should_display_level(&MinLevel::Info, &MinLevel::Error));
}

// ==================== Command Handler Integration Tests ====================

use crate::dynamic::{CallResponse, DynamicMessage};
use crate::ShellState;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;
use serial_test::serial;

/// Helper to properly clean up ShellState after test.
/// Stops the monitor actor and waits for cleanup.
async fn cleanup_shell_state(state: ShellState) {
    if let Some(monitor_ref) = &state.monitor_actor {
        monitor_ref.stop(None);
        // Give the actor time to stop and unregister
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Test actor that supports DynamicMessage for shell interaction tests.
struct TestDynamicActor {
    name: String,
}

impl Actor for TestDynamicActor {
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
                            let _ = reply.send(CallResponse::Error(format!("Unknown: {}", cmd)));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error("No command".to_string()));
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

// ==================== cmd_actors Tests ====================

#[tokio::test]
#[serial]
async fn cmd_actors___empty_registry___prints_no_actors_message() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    // Execute actors command - should not error even with empty registry
    let result = state.execute(ShellCommand::Actors).await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_actors___with_registered_actor___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_cmd_actors_actor".to_string()),
        TestDynamicActor {
            name: "test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = state.execute(ShellCommand::Actors).await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

// ==================== cmd_registry Tests ====================

#[tokio::test]
#[serial]
async fn cmd_registry___with_registered_actor___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_cmd_registry_actor".to_string()),
        TestDynamicActor {
            name: "registry_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = state.execute(ShellCommand::Registry).await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

// ==================== cmd_info Tests ====================

#[tokio::test]
#[serial]
async fn cmd_info___existing_actor___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_cmd_info_actor".to_string()),
        TestDynamicActor {
            name: "info_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = state
        .execute(ShellCommand::Info {
            actor: "test_cmd_info_actor".to_string(),
        })
        .await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_info___nonexistent_actor___returns_error() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state
        .execute(ShellCommand::Info {
            actor: "nonexistent_actor_xyz".to_string(),
        })
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not found") || err.to_string().contains("Actor"));
    cleanup_shell_state(state).await;
}

// ==================== cmd_pg_list Tests ====================

#[tokio::test]
#[serial]
async fn cmd_pg_list___empty_groups___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state.execute(ShellCommand::PgList).await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_pg_list___with_group___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_pg_list_actor".to_string()),
        TestDynamicActor {
            name: "pg_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    ractor::pg::join("test_pg_list_group".to_string(), vec![actor_ref.get_cell()]);

    let result = state.execute(ShellCommand::PgList).await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

// ==================== cmd_pg_members Tests ====================

#[tokio::test]
#[serial]
async fn cmd_pg_members___existing_group___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_pg_members_actor".to_string()),
        TestDynamicActor {
            name: "pg_members_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    ractor::pg::join(
        "test_pg_members_group".to_string(),
        vec![actor_ref.get_cell()],
    );

    let result = state
        .execute(ShellCommand::PgMembers {
            group: "test_pg_members_group".to_string(),
        })
        .await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_pg_members___nonexistent_group___returns_ok_with_empty() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state
        .execute(ShellCommand::PgMembers {
            group: "nonexistent_group_xyz".to_string(),
        })
        .await;

    // Should succeed but show no members
    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

// ==================== cmd_send Tests ====================

#[tokio::test]
#[serial]
async fn cmd_send___dynamic_actor_valid_json___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_cmd_send_actor".to_string()),
        TestDynamicActor {
            name: "send_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = state
        .execute(ShellCommand::Send {
            actor: "test_cmd_send_actor".to_string(),
            message: r#"{"command": "increment"}"#.to_string(),
        })
        .await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_send___nonexistent_actor___returns_ok_with_error_message() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    // cmd_send prints error but doesn't return Err for missing actors
    let result = state
        .execute(ShellCommand::Send {
            actor: "nonexistent_send_actor".to_string(),
            message: r#"{"command": "test"}"#.to_string(),
        })
        .await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_send___invalid_json___returns_ok_with_parse_error() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_send_invalid_json_actor".to_string()),
        TestDynamicActor {
            name: "invalid_json_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    // cmd_send handles parse errors gracefully
    let result = state
        .execute(ShellCommand::Send {
            actor: "test_send_invalid_json_actor".to_string(),
            message: "not valid json {{".to_string(),
        })
        .await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

// ==================== cmd_call Tests ====================

#[tokio::test]
#[serial]
async fn cmd_call___dynamic_actor_valid_json___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_cmd_call_actor".to_string()),
        TestDynamicActor {
            name: "call_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = state
        .execute(ShellCommand::Call {
            actor: "test_cmd_call_actor".to_string(),
            message: r#"{"command": "ping"}"#.to_string(),
        })
        .await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_call___nonexistent_actor___returns_ok_with_error_message() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state
        .execute(ShellCommand::Call {
            actor: "nonexistent_call_actor".to_string(),
            message: r#"{"command": "test"}"#.to_string(),
        })
        .await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

// ==================== cmd_stop Tests ====================

#[tokio::test]
#[serial]
async fn cmd_stop___existing_actor___stops_actor() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (_actor_ref, handle) = Actor::spawn(
        Some("test_cmd_stop_actor".to_string()),
        TestDynamicActor {
            name: "stop_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = state
        .execute(ShellCommand::Stop {
            actor: "test_cmd_stop_actor".to_string(),
        })
        .await;

    assert!(result.is_ok());

    // Wait for actor to stop
    let _ = tokio::time::timeout(std::time::Duration::from_millis(500), handle).await;
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_stop___nonexistent_actor___returns_error() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state
        .execute(ShellCommand::Stop {
            actor: "nonexistent_stop_actor".to_string(),
        })
        .await;

    assert!(result.is_err());
    cleanup_shell_state(state).await;
}

// ==================== cmd_monitor Tests ====================

#[tokio::test]
#[serial]
async fn cmd_monitor___valid_actor___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_cmd_monitor_actor".to_string()),
        TestDynamicActor {
            name: "monitor_test".to_string(),
        },
        (),
    )
    .await
    .expect("Failed to spawn actor");

    let result = state
        .execute(ShellCommand::Monitor {
            actor: "test_cmd_monitor_actor".to_string(),
        })
        .await;

    assert!(result.is_ok());
    actor_ref.stop(None);
    cleanup_shell_state(state).await;
}

// ==================== cmd_unmonitor Tests ====================

#[tokio::test]
#[serial]
async fn cmd_unmonitor___any_actor___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    // Unmonitoring an actor that wasn't monitored is still OK
    let result = state
        .execute(ShellCommand::Unmonitor {
            actor: "some_actor".to_string(),
        })
        .await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

// ==================== cmd_monitors Tests ====================

#[tokio::test]
#[serial]
async fn cmd_monitors___no_monitored___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state.execute(ShellCommand::Monitors).await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

// ==================== cmd_help Tests ====================

#[tokio::test]
#[serial]
async fn cmd_help___no_arg___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state.execute(ShellCommand::Help { command: None }).await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_help___with_command_arg___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state
        .execute(ShellCommand::Help {
            command: Some("actors".to_string()),
        })
        .await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn cmd_help___unknown_command___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state
        .execute(ShellCommand::Help {
            command: Some("unknowncommand".to_string()),
        })
        .await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

// ==================== cmd_stats Tests ====================

#[tokio::test]
#[serial]
async fn cmd_stats___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state.execute(ShellCommand::Stats).await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

// ==================== cmd_nodes Tests ====================

#[tokio::test]
#[serial]
async fn cmd_nodes___no_connections___returns_ok() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let result = state.execute(ShellCommand::Nodes).await;

    assert!(result.is_ok());
    cleanup_shell_state(state).await;
}

// ==================== cmd_exit Tests ====================

#[tokio::test]
#[serial]
async fn cmd_exit___sets_should_exit_flag() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    assert!(!state.should_exit);

    let result = state.execute(ShellCommand::Exit).await;

    assert!(result.is_ok());
    assert!(state.should_exit);
    cleanup_shell_state(state).await;
}

// ==================== build_prompt Tests ====================

#[tokio::test]
#[serial]
async fn build_prompt___local_node___contains_local() {
    let state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");

    let prompt = state.build_prompt();

    // Should contain "local" since no remote node is selected
    assert!(prompt.contains("local") || prompt.contains("ractor"));
    cleanup_shell_state(state).await;
}

#[tokio::test]
#[serial]
async fn build_prompt___with_current_node___contains_node_name() {
    let mut state = ShellState::new_quiet()
        .await
        .expect("Failed to create shell state");
    state.current_node = Some("test_node".to_string());

    let prompt = state.build_prompt();

    assert!(prompt.contains("test_node"));
    cleanup_shell_state(state).await;
}

// ==================== looks_like_typed_message Tests ====================

#[test]
fn looks_like_typed_message___variant_with_json___returns_true() {
    assert!(ShellState::looks_like_typed_message("Ping {}"));
    assert!(ShellState::looks_like_typed_message("GetStatus {}"));
    assert!(ShellState::looks_like_typed_message(
        "DoSomething {\"arg\": 1}"
    ));
}

#[test]
fn looks_like_typed_message___plain_json___returns_false() {
    assert!(!ShellState::looks_like_typed_message(
        r#"{"command": "ping"}"#
    ));
    assert!(!ShellState::looks_like_typed_message("123"));
    assert!(!ShellState::looks_like_typed_message("lowercase {}"));
}

// ==================== parse_typed_message Tests ====================

#[test]
fn parse_typed_message___valid_variant_with_empty_object___returns_parts() {
    let result = ShellState::parse_typed_message("Ping {}");

    assert!(result.is_ok());
    let (variant, args) = result.unwrap();
    assert_eq!(variant, "Ping");
    assert_eq!(args, json!({}));
}

#[test]
fn parse_typed_message___valid_variant_with_args___returns_parts() {
    let result = ShellState::parse_typed_message("GetValue {\"key\": \"test\"}");

    assert!(result.is_ok());
    let (variant, args) = result.unwrap();
    assert_eq!(variant, "GetValue");
    assert_eq!(args, json!({"key": "test"}));
}

#[test]
fn parse_typed_message___invalid_json_args___returns_error() {
    // The function fails when the JSON portion is malformed
    let result = ShellState::parse_typed_message("Variant {invalid json}");

    assert!(result.is_err());
}

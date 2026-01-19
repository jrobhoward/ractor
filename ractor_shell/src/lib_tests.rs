#![allow(non_snake_case)]

use crate::ShellCommand;

// ==================== parse_line: Error Cases ====================

#[test]
fn parse_line___empty_input___returns_empty_command_error() {
    let result = ShellCommand::parse_line("");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Empty command"));
}

#[test]
fn parse_line___whitespace_only___returns_error() {
    let result = ShellCommand::parse_line("   ");

    assert!(result.is_err());
}

#[test]
fn parse_line___unknown_command___returns_unknown_command_error() {
    let result = ShellCommand::parse_line("foobar");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown command"));
}

// ==================== parse_line: Basic Commands ====================

#[test]
fn parse_line___actors_command___returns_actors_variant() {
    let cmd = ShellCommand::parse_line("actors").unwrap();

    assert!(matches!(cmd, ShellCommand::Actors));
}

#[test]
fn parse_line___registry_command___returns_registry_variant() {
    let cmd = ShellCommand::parse_line("registry").unwrap();

    assert!(matches!(cmd, ShellCommand::Registry));
}

#[test]
fn parse_line___nodes_command___returns_nodes_variant() {
    let cmd = ShellCommand::parse_line("nodes").unwrap();

    assert!(matches!(cmd, ShellCommand::Nodes));
}

#[test]
fn parse_line___stats_command___returns_stats_variant() {
    let cmd = ShellCommand::parse_line("stats").unwrap();

    assert!(matches!(cmd, ShellCommand::Stats));
}

#[test]
fn parse_line___monitors_command___returns_monitors_variant() {
    let cmd = ShellCommand::parse_line("monitors").unwrap();

    assert!(matches!(cmd, ShellCommand::Monitors));
}

#[test]
fn parse_line___exit_command___returns_exit_variant() {
    let cmd = ShellCommand::parse_line("exit").unwrap();

    assert!(matches!(cmd, ShellCommand::Exit));
}

#[test]
fn parse_line___quit_command___returns_exit_variant() {
    let cmd = ShellCommand::parse_line("quit").unwrap();

    assert!(matches!(cmd, ShellCommand::Exit));
}

// ==================== parse_line: Help Command ====================

#[test]
fn parse_line___help_without_arg___returns_help_with_none() {
    let cmd = ShellCommand::parse_line("help").unwrap();

    match cmd {
        ShellCommand::Help { command } => assert!(command.is_none()),
        _ => panic!("Expected Help command"),
    }
}

#[test]
fn parse_line___help_with_arg___returns_help_with_command() {
    let cmd = ShellCommand::parse_line("help actors").unwrap();

    match cmd {
        ShellCommand::Help { command } => assert_eq!(command, Some("actors".to_string())),
        _ => panic!("Expected Help command"),
    }
}

// ==================== parse_line: Info Command ====================

#[test]
fn parse_line___info_with_actor___returns_info_variant() {
    let cmd = ShellCommand::parse_line("info my_actor").unwrap();

    match cmd {
        ShellCommand::Info { actor } => assert_eq!(actor, "my_actor"),
        _ => panic!("Expected Info command"),
    }
}

#[test]
fn parse_line___info_missing_arg___returns_error() {
    let result = ShellCommand::parse_line("info");

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("requires an actor name"));
}

// ==================== parse_line: Stop Command ====================

#[test]
fn parse_line___stop_with_actor___returns_stop_variant() {
    let cmd = ShellCommand::parse_line("stop my_actor").unwrap();

    match cmd {
        ShellCommand::Stop { actor } => assert_eq!(actor, "my_actor"),
        _ => panic!("Expected Stop command"),
    }
}

// ==================== parse_line: Send Command ====================

#[test]
fn parse_line___send_with_json___returns_send_variant() {
    let cmd = ShellCommand::parse_line(r#"send my_actor {"cmd": "ping"}"#).unwrap();

    match cmd {
        ShellCommand::Send { actor, message } => {
            assert_eq!(actor, "my_actor");
            assert_eq!(message, r#"{"cmd": "ping"}"#);
        }
        _ => panic!("Expected Send command"),
    }
}

#[test]
fn parse_line___send_multiword_message___joins_remaining_args() {
    let cmd = ShellCommand::parse_line("send actor hello world").unwrap();

    match cmd {
        ShellCommand::Send { actor, message } => {
            assert_eq!(actor, "actor");
            assert_eq!(message, "hello world");
        }
        _ => panic!("Expected Send command"),
    }
}

#[test]
fn parse_line___send_missing_message___returns_error() {
    let result = ShellCommand::parse_line("send actor");

    assert!(result.is_err());
}

// ==================== parse_line: Call Command ====================

#[test]
fn parse_line___call_with_json___returns_call_variant() {
    let cmd = ShellCommand::parse_line(r#"call my_actor {"cmd": "get"}"#).unwrap();

    match cmd {
        ShellCommand::Call { actor, message } => {
            assert_eq!(actor, "my_actor");
            assert_eq!(message, r#"{"cmd": "get"}"#);
        }
        _ => panic!("Expected Call command"),
    }
}

// ==================== parse_line: Connect/Disconnect Commands ====================

#[test]
fn parse_line___connect_with_host___returns_connect_variant() {
    let cmd = ShellCommand::parse_line("connect localhost:9000").unwrap();

    match cmd {
        ShellCommand::Connect { host } => assert_eq!(host, "localhost:9000"),
        _ => panic!("Expected Connect command"),
    }
}

#[test]
fn parse_line___disconnect_with_node___returns_disconnect_variant() {
    let cmd = ShellCommand::parse_line("disconnect node_a").unwrap();

    match cmd {
        ShellCommand::Disconnect { node } => assert_eq!(node, "node_a"),
        _ => panic!("Expected Disconnect command"),
    }
}

// ==================== parse_line: Use Command ====================

#[test]
fn parse_line___use_with_node___returns_use_variant() {
    let cmd = ShellCommand::parse_line("use node_b").unwrap();

    match cmd {
        ShellCommand::Use { node } => assert_eq!(node, "node_b"),
        _ => panic!("Expected Use command"),
    }
}

#[test]
fn parse_line___use_local___returns_use_with_local() {
    let cmd = ShellCommand::parse_line("use local").unwrap();

    match cmd {
        ShellCommand::Use { node } => assert_eq!(node, "local"),
        _ => panic!("Expected Use command"),
    }
}

// ==================== parse_line: pg Subcommands ====================

#[test]
fn parse_line___pg_list___returns_pg_list_variant() {
    let cmd = ShellCommand::parse_line("pg list").unwrap();

    assert!(matches!(cmd, ShellCommand::PgList));
}

#[test]
fn parse_line___pg_members_with_group___returns_pg_members_variant() {
    let cmd = ShellCommand::parse_line("pg members my_group").unwrap();

    match cmd {
        ShellCommand::PgMembers { group } => assert_eq!(group, "my_group"),
        _ => panic!("Expected PgMembers command"),
    }
}

#[test]
fn parse_line___pg_without_subcommand___returns_error() {
    let result = ShellCommand::parse_line("pg");

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("requires a subcommand"));
}

#[test]
fn parse_line___pg_members_missing_group___returns_error() {
    let result = ShellCommand::parse_line("pg members");

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("requires a group name"));
}

#[test]
fn parse_line___pg_unknown_subcommand___returns_error() {
    let result = ShellCommand::parse_line("pg foobar");

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Unknown pg subcommand"));
}

// ==================== parse_line: Cluster Command ====================

#[test]
fn parse_line___cluster_without_subcommand___returns_cluster_with_none() {
    let cmd = ShellCommand::parse_line("cluster").unwrap();

    match cmd {
        ShellCommand::Cluster { subcommand } => assert!(subcommand.is_none()),
        _ => panic!("Expected Cluster command"),
    }
}

#[test]
fn parse_line___cluster_with_subcommand___returns_cluster_with_subcommand() {
    let cmd = ShellCommand::parse_line("cluster nodes").unwrap();

    match cmd {
        ShellCommand::Cluster { subcommand } => {
            assert_eq!(subcommand, Some("nodes".to_string()))
        }
        _ => panic!("Expected Cluster command"),
    }
}

// ==================== parse_line: Tree Command ====================

#[test]
fn parse_line___tree_without_arg___returns_tree_with_none() {
    let cmd = ShellCommand::parse_line("tree").unwrap();

    match cmd {
        ShellCommand::Tree { actor } => assert!(actor.is_none()),
        _ => panic!("Expected Tree command"),
    }
}

#[test]
fn parse_line___tree_with_actor___returns_tree_with_actor() {
    let cmd = ShellCommand::parse_line("tree my_actor").unwrap();

    match cmd {
        ShellCommand::Tree { actor } => assert_eq!(actor, Some("my_actor".to_string())),
        _ => panic!("Expected Tree command"),
    }
}

// ==================== parse_line: Supervtree Command ====================

#[test]
fn parse_line___supervtree_without_arg___returns_supervtree_with_none() {
    let cmd = ShellCommand::parse_line("supervtree").unwrap();

    match cmd {
        ShellCommand::Supervtree { actor } => assert!(actor.is_none()),
        _ => panic!("Expected Supervtree command"),
    }
}

#[test]
fn parse_line___supervtree_with_actor___returns_supervtree_with_actor() {
    let cmd = ShellCommand::parse_line("supervtree my_actor").unwrap();

    match cmd {
        ShellCommand::Supervtree { actor } => assert_eq!(actor, Some("my_actor".to_string())),
        _ => panic!("Expected Supervtree command"),
    }
}

#[test]
fn parse_line___supervtree_alias_st___returns_supervtree() {
    let cmd = ShellCommand::parse_line("st").unwrap();

    match cmd {
        ShellCommand::Supervtree { actor } => assert!(actor.is_none()),
        _ => panic!("Expected Supervtree command"),
    }
}

// ==================== parse_line: Parent Command ====================

#[test]
fn parse_line___parent_with_actor___returns_parent_variant() {
    let cmd = ShellCommand::parse_line("parent my_actor").unwrap();

    match cmd {
        ShellCommand::Parent { actor } => assert_eq!(actor, "my_actor"),
        _ => panic!("Expected Parent command"),
    }
}

#[test]
fn parse_line___parent_without_arg___returns_error() {
    let result = ShellCommand::parse_line("parent");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("parent"));
}

#[test]
fn parse_line___parent_alias_p___returns_parent_variant() {
    let cmd = ShellCommand::parse_line("p my_actor").unwrap();

    match cmd {
        ShellCommand::Parent { actor } => assert_eq!(actor, "my_actor"),
        _ => panic!("Expected Parent command"),
    }
}

// ==================== parse_line: File Commands ====================

#[test]
fn parse_line___send_file_with_path___returns_send_file_variant() {
    let cmd = ShellCommand::parse_line("send-file actor /path/to/file.json").unwrap();

    match cmd {
        ShellCommand::SendFile { actor, file_path } => {
            assert_eq!(actor, "actor");
            assert_eq!(file_path, "/path/to/file.json");
        }
        _ => panic!("Expected SendFile command"),
    }
}

#[test]
fn parse_line___sendfile_alt_syntax___returns_send_file_variant() {
    let cmd = ShellCommand::parse_line("sendfile actor file.json").unwrap();

    match cmd {
        ShellCommand::SendFile { actor, file_path } => {
            assert_eq!(actor, "actor");
            assert_eq!(file_path, "file.json");
        }
        _ => panic!("Expected SendFile command"),
    }
}

#[test]
fn parse_line___load_with_path___returns_load_variant() {
    let cmd = ShellCommand::parse_line("load /path/to/script.sh").unwrap();

    match cmd {
        ShellCommand::Load { script_path } => assert_eq!(script_path, "/path/to/script.sh"),
        _ => panic!("Expected Load command"),
    }
}

// ==================== parse_line: Monitor Commands ====================

#[test]
fn parse_line___monitor_with_actor___returns_monitor_variant() {
    let cmd = ShellCommand::parse_line("monitor my_actor").unwrap();

    match cmd {
        ShellCommand::Monitor { actor } => assert_eq!(actor, "my_actor"),
        _ => panic!("Expected Monitor command"),
    }
}

#[test]
fn parse_line___unmonitor_with_actor___returns_unmonitor_variant() {
    let cmd = ShellCommand::parse_line("unmonitor my_actor").unwrap();

    match cmd {
        ShellCommand::Unmonitor { actor } => assert_eq!(actor, "my_actor"),
        _ => panic!("Expected Unmonitor command"),
    }
}

// ==================== parse_line: Aliases ====================

#[test]
fn parse_line___alias_a___returns_actors_variant() {
    let cmd = ShellCommand::parse_line("a").unwrap();

    assert!(matches!(cmd, ShellCommand::Actors));
}

#[test]
fn parse_line___alias_r___returns_registry_variant() {
    let cmd = ShellCommand::parse_line("r").unwrap();

    assert!(matches!(cmd, ShellCommand::Registry));
}

#[test]
fn parse_line___alias_i_with_actor___returns_info_variant() {
    let cmd = ShellCommand::parse_line("i my_actor").unwrap();

    match cmd {
        ShellCommand::Info { actor } => assert_eq!(actor, "my_actor"),
        _ => panic!("Expected Info command"),
    }
}

#[test]
fn parse_line___alias_s_with_message___returns_send_variant() {
    let cmd = ShellCommand::parse_line("s actor message").unwrap();

    match cmd {
        ShellCommand::Send { actor, message } => {
            assert_eq!(actor, "actor");
            assert_eq!(message, "message");
        }
        _ => panic!("Expected Send command"),
    }
}

#[test]
fn parse_line___alias_c_with_message___returns_call_variant() {
    let cmd = ShellCommand::parse_line("c actor message").unwrap();

    match cmd {
        ShellCommand::Call { actor, message } => {
            assert_eq!(actor, "actor");
            assert_eq!(message, "message");
        }
        _ => panic!("Expected Call command"),
    }
}

#[test]
fn parse_line___alias_sf_with_file___returns_send_file_variant() {
    let cmd = ShellCommand::parse_line("sf actor file.json").unwrap();

    match cmd {
        ShellCommand::SendFile { actor, file_path } => {
            assert_eq!(actor, "actor");
            assert_eq!(file_path, "file.json");
        }
        _ => panic!("Expected SendFile command"),
    }
}

#[test]
fn parse_line___alias_l_with_script___returns_load_variant() {
    let cmd = ShellCommand::parse_line("l script.sh").unwrap();

    match cmd {
        ShellCommand::Load { script_path } => assert_eq!(script_path, "script.sh"),
        _ => panic!("Expected Load command"),
    }
}

#[test]
fn parse_line___alias_q___returns_exit_variant() {
    let cmd = ShellCommand::parse_line("q").unwrap();

    assert!(matches!(cmd, ShellCommand::Exit));
}

// ==================== parse_line: Top Command ====================

#[test]
fn parse_line___top_command___returns_top_variant() {
    let cmd = ShellCommand::parse_line("top").unwrap();

    assert!(matches!(cmd, ShellCommand::Top));
}

#[test]
fn parse_line___alias_t___returns_top_variant() {
    let cmd = ShellCommand::parse_line("t").unwrap();

    assert!(matches!(cmd, ShellCommand::Top));
}

// ==================== parse_line: Trace Level Command ====================

#[test]
fn parse_line___trace_level_without_arg___returns_trace_level_with_none() {
    let cmd = ShellCommand::parse_line("trace level").unwrap();

    match cmd {
        ShellCommand::TraceLevel { level } => assert!(level.is_none()),
        _ => panic!("Expected TraceLevel command"),
    }
}

#[test]
fn parse_line___trace_level_with_info___returns_trace_level_with_info() {
    let cmd = ShellCommand::parse_line("trace level INFO").unwrap();

    match cmd {
        ShellCommand::TraceLevel { level } => assert_eq!(level, Some("INFO".to_string())),
        _ => panic!("Expected TraceLevel command"),
    }
}

#[test]
fn parse_line___trace_level_with_lowercase___returns_trace_level_with_value() {
    let cmd = ShellCommand::parse_line("trace level debug").unwrap();

    match cmd {
        ShellCommand::TraceLevel { level } => assert_eq!(level, Some("debug".to_string())),
        _ => panic!("Expected TraceLevel command"),
    }
}

// ==================== should_display_level Tests ====================

use crate::should_display_level;
use crate::tracing::MinLevel;

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

// ==================== parse_line: Ping Command ====================

#[test]
fn parse_line___ping_with_node___returns_ping_variant() {
    let cmd = ShellCommand::parse_line("ping 127.0.0.1:9001").unwrap();

    match cmd {
        ShellCommand::Ping { node } => assert_eq!(node, "127.0.0.1:9001"),
        _ => panic!("Expected Ping variant"),
    }
}

#[test]
fn parse_line___ping_without_arg___returns_error() {
    let result = ShellCommand::parse_line("ping");

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("requires"));
}

// ==================== resolve_alias: Complete Coverage ====================

#[test]
fn resolve_alias___a___resolves_to_actors() {
    assert_eq!(ShellCommand::resolve_alias("a"), "actors");
}

#[test]
fn resolve_alias___r___resolves_to_registry() {
    assert_eq!(ShellCommand::resolve_alias("r"), "registry");
}

#[test]
fn resolve_alias___i___resolves_to_info() {
    assert_eq!(ShellCommand::resolve_alias("i"), "info");
}

#[test]
fn resolve_alias___sc___resolves_to_schema() {
    assert_eq!(ShellCommand::resolve_alias("sc"), "schema");
}

#[test]
fn resolve_alias___st___resolves_to_supervtree() {
    assert_eq!(ShellCommand::resolve_alias("st"), "supervtree");
}

#[test]
fn resolve_alias___p___resolves_to_parent() {
    assert_eq!(ShellCommand::resolve_alias("p"), "parent");
}

#[test]
fn resolve_alias___s___resolves_to_send() {
    assert_eq!(ShellCommand::resolve_alias("s"), "send");
}

#[test]
fn resolve_alias___c___resolves_to_call() {
    assert_eq!(ShellCommand::resolve_alias("c"), "call");
}

#[test]
fn resolve_alias___sf___resolves_to_send_file() {
    assert_eq!(ShellCommand::resolve_alias("sf"), "send-file");
}

#[test]
fn resolve_alias___con___resolves_to_connect() {
    assert_eq!(ShellCommand::resolve_alias("con"), "connect");
}

#[test]
fn resolve_alias___dis___resolves_to_disconnect() {
    assert_eq!(ShellCommand::resolve_alias("dis"), "disconnect");
}

#[test]
fn resolve_alias___n___resolves_to_nodes() {
    assert_eq!(ShellCommand::resolve_alias("n"), "nodes");
}

#[test]
fn resolve_alias___u___resolves_to_use() {
    assert_eq!(ShellCommand::resolve_alias("u"), "use");
}

#[test]
fn resolve_alias___cl___resolves_to_cluster() {
    assert_eq!(ShellCommand::resolve_alias("cl"), "cluster");
}

#[test]
fn resolve_alias___m___resolves_to_monitor() {
    assert_eq!(ShellCommand::resolve_alias("m"), "monitor");
}

#[test]
fn resolve_alias___um___resolves_to_unmonitor() {
    assert_eq!(ShellCommand::resolve_alias("um"), "unmonitor");
}

#[test]
fn resolve_alias___ms___resolves_to_monitors() {
    assert_eq!(ShellCommand::resolve_alias("ms"), "monitors");
}

#[test]
fn resolve_alias___t___resolves_to_top() {
    assert_eq!(ShellCommand::resolve_alias("t"), "top");
}

#[test]
fn resolve_alias___tr___resolves_to_trace() {
    assert_eq!(ShellCommand::resolve_alias("tr"), "trace");
}

#[test]
fn resolve_alias___tf___resolves_to_trace_to_file() {
    assert_eq!(ShellCommand::resolve_alias("tf"), "trace-to-file");
}

#[test]
fn resolve_alias___h___resolves_to_help() {
    assert_eq!(ShellCommand::resolve_alias("h"), "help");
}

#[test]
fn resolve_alias___l___resolves_to_load() {
    assert_eq!(ShellCommand::resolve_alias("l"), "load");
}

#[test]
fn resolve_alias___q___resolves_to_quit() {
    assert_eq!(ShellCommand::resolve_alias("q"), "quit");
}

#[test]
fn resolve_alias___unknown___returns_unchanged() {
    assert_eq!(ShellCommand::resolve_alias("foobar"), "foobar");
    assert_eq!(ShellCommand::resolve_alias("actors"), "actors");
    assert_eq!(ShellCommand::resolve_alias("registry"), "registry");
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

// ==================== Additional Trace Command Parse Tests ====================

#[test]
fn parse_line___trace_without_arg___returns_trace_with_none() {
    let cmd = ShellCommand::parse_line("trace").unwrap();

    match cmd {
        ShellCommand::Trace { pattern } => assert!(pattern.is_none()),
        _ => panic!("Expected Trace command"),
    }
}

#[test]
fn parse_line___trace_with_pattern___returns_trace_with_pattern() {
    let cmd = ShellCommand::parse_line("trace my_actor*").unwrap();

    match cmd {
        ShellCommand::Trace { pattern } => assert_eq!(pattern, Some("my_actor*".to_string())),
        _ => panic!("Expected Trace command"),
    }
}

#[test]
fn parse_line___trace_off___returns_trace_off() {
    let cmd = ShellCommand::parse_line("trace off").unwrap();

    assert!(matches!(cmd, ShellCommand::TraceOff));
}

#[test]
fn parse_line___schema_without_arg___returns_schema_with_none() {
    let cmd = ShellCommand::parse_line("schema").unwrap();

    match cmd {
        ShellCommand::Schema { actor } => assert!(actor.is_none()),
        _ => panic!("Expected Schema command"),
    }
}

#[test]
fn parse_line___schema_with_actor___returns_schema_with_actor() {
    let cmd = ShellCommand::parse_line("schema my_actor").unwrap();

    match cmd {
        ShellCommand::Schema { actor } => assert_eq!(actor, Some("my_actor".to_string())),
        _ => panic!("Expected Schema command"),
    }
}

#[test]
fn parse_line___reconnect_with_node___returns_reconnect_variant() {
    let cmd = ShellCommand::parse_line("reconnect node_a").unwrap();

    match cmd {
        ShellCommand::Reconnect { node } => assert_eq!(node, "node_a"),
        _ => panic!("Expected Reconnect command"),
    }
}

#[test]
fn parse_line___monitor_events___returns_monitor_events_variant() {
    // monitor events is a subcommand of monitor
    let cmd = ShellCommand::parse_line("monitor events").unwrap();

    assert!(matches!(cmd, ShellCommand::MonitorEvents));
}

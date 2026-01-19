#![allow(non_snake_case)]

use super::ShellCommand;

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

// ==================== parse_line: Additional Trace Commands ====================

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
    let cmd = ShellCommand::parse_line("monitor events").unwrap();

    assert!(matches!(cmd, ShellCommand::MonitorEvents));
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

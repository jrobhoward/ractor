#![allow(non_snake_case)]

use super::{get_known_process_groups, update_completer_state, ShellHelper};
use rustyline::completion::Completer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::Context;

/// Helper macro to create a context for testing
macro_rules! ctx {
    () => {{
        static HISTORY: std::sync::LazyLock<DefaultHistory> =
            std::sync::LazyLock::new(DefaultHistory::new);
        Context::new(&*HISTORY)
    }};
}

// ==================== ShellHelper Construction Tests ====================

#[test]
fn ShellHelper___new___creates_empty_helper() {
    let helper = ShellHelper::new();

    assert!(helper.actor_names.is_empty());
    assert!(helper.process_groups.is_empty());
    assert!(helper.node_names.is_empty());
}

#[test]
fn ShellHelper___default___creates_empty_helper() {
    let helper = ShellHelper::default();

    assert!(helper.actor_names.is_empty());
    assert!(helper.process_groups.is_empty());
    assert!(helper.node_names.is_empty());
}

// ==================== State Update Tests ====================

#[test]
fn ShellHelper___update_actors___stores_actor_names() {
    let mut helper = ShellHelper::new();
    let actors = vec!["actor1".to_string(), "actor2".to_string()];

    helper.update_actors(actors.clone());

    assert_eq!(helper.actor_names, actors);
}

#[test]
fn ShellHelper___update_groups___stores_process_groups() {
    let mut helper = ShellHelper::new();
    let groups = vec!["group1".to_string(), "group2".to_string()];

    helper.update_groups(groups.clone());

    assert_eq!(helper.process_groups, groups);
}

#[test]
fn ShellHelper___update_nodes___stores_node_names() {
    let mut helper = ShellHelper::new();
    let nodes = vec!["node1".to_string(), "node2".to_string()];

    helper.update_nodes(nodes.clone());

    assert_eq!(helper.node_names, nodes);
}

// ==================== Empty Input Completion Tests ====================

#[test]
fn complete___empty_input___suggests_all_commands() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("", 0, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    // Should include common commands
    assert!(candidates.iter().any(|c| c.display == "help"));
    assert!(candidates.iter().any(|c| c.display == "actors"));
    assert!(candidates.iter().any(|c| c.display == "registry"));
    assert!(candidates.iter().any(|c| c.display == "info"));
    assert!(candidates.iter().any(|c| c.display == "send"));
    assert!(candidates.iter().any(|c| c.display == "call"));
    assert!(candidates.iter().any(|c| c.display == "quit"));
}

// ==================== Command Completion Tests ====================

#[test]
fn complete___partial_help_command___suggests_help() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("he", 2, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    assert!(candidates.iter().any(|c| c.display == "help"));
}

#[test]
fn complete___partial_actors___suggests_actors() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("act", 3, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    assert!(candidates.iter().any(|c| c.display == "actors"));
}

#[test]
fn complete___partial_trace___suggests_trace_commands() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("tra", 3, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    assert!(candidates.iter().any(|c| c.display == "trace"));
    assert!(candidates.iter().any(|c| c.display == "trace-to-file"));
}

#[test]
fn complete___partial_monitor___suggests_monitor_commands() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("mon", 3, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    assert!(candidates.iter().any(|c| c.display == "monitor"));
    assert!(candidates.iter().any(|c| c.display == "monitors"));
}

// ==================== Alias Completion Tests ====================

#[test]
fn complete___partial_alias_a___suggests_a_alias() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("a", 1, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    // 'a' is an alias for 'actors'
    assert!(candidates.iter().any(|c| c.display == "a"));
    // Also matches 'actors'
    assert!(candidates.iter().any(|c| c.display == "actors"));
}

#[test]
fn complete___alias_r___suggests_r_and_registry() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("r", 1, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    assert!(candidates.iter().any(|c| c.display == "r"));
    assert!(candidates.iter().any(|c| c.display == "registry"));
}

#[test]
fn complete___alias_prefix_s___suggests_multiple() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("s", 1, &ctx!()).unwrap();

    assert_eq!(pos, 0);
    // 's' alias, 'send', 'send-file', 'sendfile', 'stop', 'stats', 'supervtree', 'sc', 'st', 'sf'
    assert!(candidates.iter().any(|c| c.display == "s"));
    assert!(candidates.iter().any(|c| c.display == "send"));
    assert!(candidates.iter().any(|c| c.display == "stop"));
    assert!(candidates.iter().any(|c| c.display == "stats"));
}

// ==================== pg Subcommand Completion Tests ====================

#[test]
fn complete___pg_with_partial_subcommand___suggests_members() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("pg m", 4, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "members"));
}

#[test]
fn complete___pg_with_partial_l___suggests_list() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("pg l", 4, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "list"));
}

#[test]
fn complete___pg_members_with_groups___suggests_group_names() {
    let mut helper = ShellHelper::new();
    helper.update_groups(vec!["demo_group".to_string(), "ping_pong".to_string()]);

    let (_pos, candidates) = helper.complete("pg members ", 11, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "demo_group"));
    assert!(candidates.iter().any(|c| c.display == "ping_pong"));
}

#[test]
fn complete___pg_members_with_partial___filters_groups() {
    let mut helper = ShellHelper::new();
    helper.update_groups(vec!["demo_group".to_string(), "ping_pong".to_string()]);

    let (_pos, candidates) = helper.complete("pg members de", 13, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "demo_group"));
    assert!(!candidates.iter().any(|c| c.display == "ping_pong"));
}

// ==================== cluster Subcommand Completion Tests ====================

#[test]
fn complete___cluster_with_space___returns_empty() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("cluster ", 8, &ctx!()).unwrap();

    // cluster subcommands only complete when not ending with space
    // (partial completion like "cluster n" works, but "cluster " returns empty)
    assert!(candidates.is_empty());
}

#[test]
fn complete___cluster_partial_n___suggests_nodes() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("cluster n", 9, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "nodes"));
}

#[test]
fn complete___cluster_partial_g___suggests_groups() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("cluster g", 9, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "groups"));
}

#[test]
fn complete___cluster_partial_a___suggests_actors() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("cluster a", 9, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "actors"));
}

// ==================== trace Subcommand Completion Tests ====================

#[test]
fn complete___trace_partial_o___suggests_off() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("trace o", 7, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "off"));
}

#[test]
fn complete___tr_alias_partial_o___suggests_off() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("tr o", 4, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "off"));
}

// ==================== Actor Name Completion Tests ====================

#[test]
fn complete___info_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec![
        "my_actor".to_string(),
        "other_actor".to_string(),
        "test_service".to_string(),
    ]);

    let (_pos, candidates) = helper.complete("info ", 5, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "my_actor"));
    assert!(candidates.iter().any(|c| c.display == "other_actor"));
    assert!(candidates.iter().any(|c| c.display == "test_service"));
}

#[test]
fn complete___info_with_partial___filters_actors() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec![
        "my_actor".to_string(),
        "other_actor".to_string(),
        "test_service".to_string(),
    ]);

    let (_pos, candidates) = helper.complete("info my", 7, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "my_actor"));
    assert!(!candidates.iter().any(|c| c.display == "other_actor"));
    assert!(!candidates.iter().any(|c| c.display == "test_service"));
}

#[test]
fn complete___stop_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["actor_to_stop".to_string()]);

    let (_pos, candidates) = helper.complete("stop ", 5, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "actor_to_stop"));
}

#[test]
fn complete___send_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["message_receiver".to_string()]);

    let (_pos, candidates) = helper.complete("send ", 5, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "message_receiver"));
}

#[test]
fn complete___call_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["rpc_service".to_string()]);

    let (_pos, candidates) = helper.complete("call ", 5, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "rpc_service"));
}

#[test]
fn complete___monitor_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["monitored_actor".to_string()]);

    let (_pos, candidates) = helper.complete("monitor ", 8, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "monitored_actor"));
}

#[test]
fn complete___unmonitor_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["unmonitored_actor".to_string()]);

    let (_pos, candidates) = helper.complete("unmonitor ", 10, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "unmonitored_actor"));
}

#[test]
fn complete___i_alias_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["target_actor".to_string()]);

    let (_pos, candidates) = helper.complete("i ", 2, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "target_actor"));
}

#[test]
fn complete___s_alias_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["send_target".to_string()]);

    let (_pos, candidates) = helper.complete("s ", 2, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "send_target"));
}

#[test]
fn complete___c_alias_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["call_target".to_string()]);

    let (_pos, candidates) = helper.complete("c ", 2, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "call_target"));
}

// ==================== Node Name Completion Tests ====================

#[test]
fn complete___use_with_nodes___suggests_node_names() {
    let mut helper = ShellHelper::new();
    helper.update_nodes(vec!["node1".to_string(), "node2".to_string()]);

    let (_pos, candidates) = helper.complete("use ", 4, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "node1"));
    assert!(candidates.iter().any(|c| c.display == "node2"));
    // Also includes 'local'
    assert!(candidates.iter().any(|c| c.display == "local"));
}

#[test]
fn complete___use_with_partial_l___suggests_local() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("use l", 5, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "local"));
}

#[test]
fn complete___use_with_partial_n___filters_nodes() {
    let mut helper = ShellHelper::new();
    helper.update_nodes(vec!["node1".to_string(), "alpha".to_string()]);

    let (_pos, candidates) = helper.complete("use n", 5, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "node1"));
    assert!(!candidates.iter().any(|c| c.display == "alpha"));
}

#[test]
fn complete___disconnect_with_nodes___suggests_node_names() {
    let mut helper = ShellHelper::new();
    helper.update_nodes(vec!["connected_node".to_string()]);

    let (_pos, candidates) = helper.complete("disconnect ", 11, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "connected_node"));
}

// ==================== Help Command Completion Tests ====================

#[test]
fn complete___help_with_partial___suggests_commands() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("help a", 6, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "actors"));
}

#[test]
fn complete___help_with_space___suggests_all_commands() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("help ", 5, &ctx!()).unwrap();

    // Should suggest commands for help topic
    assert!(candidates.iter().any(|c| c.display == "actors"));
    assert!(candidates.iter().any(|c| c.display == "help"));
    assert!(candidates.iter().any(|c| c.display == "send"));
}

// ==================== send-file Completion Tests ====================

#[test]
fn complete___sendfile_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["file_receiver".to_string()]);

    let (_pos, candidates) = helper.complete("send-file ", 10, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "file_receiver"));
}

#[test]
fn complete___sf_alias_with_actors___suggests_actor_names() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["file_receiver".to_string()]);

    let (_pos, candidates) = helper.complete("sf ", 3, &ctx!()).unwrap();

    assert!(candidates.iter().any(|c| c.display == "file_receiver"));
}

// ==================== Unknown Command Tests ====================

#[test]
fn complete___unknown_command___returns_empty() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper.complete("unknowncommand arg", 18, &ctx!()).unwrap();

    assert!(candidates.is_empty());
}

// ==================== Hinter Tests ====================

#[test]
fn hint___alias_a___shows_actors_expansion() {
    let helper = ShellHelper::new();

    let hint = helper.hint("a", 1, &ctx!());

    assert_eq!(hint, Some(" (actors)".to_string()));
}

#[test]
fn hint___alias_r___shows_registry_expansion() {
    let helper = ShellHelper::new();

    let hint = helper.hint("r", 1, &ctx!());

    assert_eq!(hint, Some(" (registry)".to_string()));
}

#[test]
fn hint___alias_i___shows_info_expansion() {
    let helper = ShellHelper::new();

    let hint = helper.hint("i", 1, &ctx!());

    assert_eq!(hint, Some(" (info)".to_string()));
}

#[test]
fn hint___alias_s___shows_send_expansion() {
    let helper = ShellHelper::new();

    let hint = helper.hint("s", 1, &ctx!());

    assert_eq!(hint, Some(" (send)".to_string()));
}

#[test]
fn hint___alias_c___shows_call_expansion() {
    let helper = ShellHelper::new();

    let hint = helper.hint("c", 1, &ctx!());

    assert_eq!(hint, Some(" (call)".to_string()));
}

#[test]
fn hint___alias_q___shows_quit_expansion() {
    let helper = ShellHelper::new();

    let hint = helper.hint("q", 1, &ctx!());

    assert_eq!(hint, Some(" (quit)".to_string()));
}

#[test]
fn hint___alias_tr___shows_trace_expansion() {
    let helper = ShellHelper::new();

    let hint = helper.hint("tr", 2, &ctx!());

    assert_eq!(hint, Some(" (trace)".to_string()));
}

#[test]
fn hint___non_alias___returns_none() {
    let helper = ShellHelper::new();

    let hint = helper.hint("actors", 6, &ctx!());

    assert_eq!(hint, None);
}

#[test]
fn hint___empty_input___returns_none() {
    let helper = ShellHelper::new();

    let hint = helper.hint("", 0, &ctx!());

    assert_eq!(hint, None);
}

#[test]
fn hint___cursor_not_at_end___returns_none() {
    let helper = ShellHelper::new();

    // Cursor at position 1 but line is 2 chars
    let hint = helper.hint("a ", 1, &ctx!());

    assert_eq!(hint, None);
}

// ==================== Highlighter Tests ====================

#[test]
fn highlight___any_line___returns_borrowed() {
    let helper = ShellHelper::new();

    let result = helper.highlight("some command", 0);

    assert_eq!(result, "some command");
}

#[test]
fn highlight_char___any_position___returns_false() {
    let helper = ShellHelper::new();

    let result = helper.highlight_char("some command", 5, false);

    assert!(!result);
}

// ==================== Utility Function Tests ====================

#[test]
fn update_completer_state___updates_all_fields() {
    let mut helper = ShellHelper::new();

    let actors = vec!["actor1".to_string(), "actor2".to_string()];
    let groups = vec!["group1".to_string()];
    let nodes = vec![
        "node1".to_string(),
        "node2".to_string(),
        "node3".to_string(),
    ];

    update_completer_state(&mut helper, actors.clone(), groups.clone(), nodes.clone());

    assert_eq!(helper.actor_names, actors);
    assert_eq!(helper.process_groups, groups);
    assert_eq!(helper.node_names, nodes);
}

#[test]
fn get_known_process_groups___returns_expected_groups() {
    let groups = get_known_process_groups();

    assert!(groups.contains(&"ping_pong".to_string()));
    assert!(groups.contains(&"ractor_shell_introspection".to_string()));
    assert!(groups.contains(&"demo_group".to_string()));
    assert!(groups.contains(&"dynamic_group".to_string()));
}

// ==================== Command Candidates Tests ====================

#[test]
fn complete___partial_command___candidates_are_sorted() {
    let helper = ShellHelper::new();

    // Partial command completion does sort results
    let (_pos, candidates) = helper.complete("s", 1, &ctx!()).unwrap();

    // Verify sorting by checking adjacent entries
    let displays: Vec<&str> = candidates.iter().map(|c| c.display.as_str()).collect();
    for window in displays.windows(2) {
        assert!(
            window[0] <= window[1],
            "Commands not sorted: {} should come before {}",
            window[0],
            window[1]
        );
    }
}

// ==================== Edge Cases ====================

#[test]
fn complete___whitespace_only___suggests_commands() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper.complete("   ", 3, &ctx!()).unwrap();

    // Whitespace-only input should be treated as empty
    assert_eq!(pos, 0);
    assert!(!candidates.is_empty());
}

#[test]
fn complete___no_matching_actors___returns_empty() {
    let mut helper = ShellHelper::new();
    helper.update_actors(vec!["alpha".to_string(), "beta".to_string()]);

    let (_pos, candidates) = helper.complete("info xyz", 8, &ctx!()).unwrap();

    assert!(candidates.is_empty());
}

#[test]
fn complete___no_matching_groups___returns_empty() {
    let mut helper = ShellHelper::new();
    helper.update_groups(vec!["group_a".to_string()]);

    let (_pos, candidates) = helper.complete("pg members xyz", 14, &ctx!()).unwrap();

    assert!(candidates.is_empty());
}

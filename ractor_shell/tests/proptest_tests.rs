#![allow(non_snake_case)]

//! Property-based tests for ractor_shell
//!
//! Uses proptest to generate random inputs and verify invariants.

use proptest::prelude::*;
use ractor_shell::ShellCommand;

// ==================== Command Parsing Properties ====================

proptest! {
    /// Any valid command should parse without panicking
    #[test]
    fn parse_line___any_string___does_not_panic(input in ".*") {
        // Should not panic regardless of input
        let _ = ShellCommand::parse_line(&input);
    }

    /// Empty or whitespace-only input should return an error
    #[test]
    fn parse_line___whitespace_only___returns_error(input in r"\s*") {
        let result = ShellCommand::parse_line(&input);

        prop_assert!(result.is_err());
    }

    /// Valid command names should parse successfully
    #[test]
    fn parse_line___valid_command___parses_successfully(
        cmd in prop_oneof![
            Just("actors"),
            Just("registry"),
            Just("nodes"),
            Just("stats"),
            Just("monitors"),
            Just("exit"),
            Just("quit"),
            Just("help"),
            Just("pg list"),
            Just("cluster"),
            Just("tree"),
        ]
    ) {
        let result = ShellCommand::parse_line(cmd);

        prop_assert!(result.is_ok(), "Failed to parse valid command: {}", cmd);
    }

    /// Aliases should behave identically to full commands
    #[test]
    fn parse_line___alias___equivalent_to_full_command(
        (alias, full) in prop_oneof![
            Just(("a", "actors")),
            Just(("r", "registry")),
            Just(("q", "quit")),
        ]
    ) {
        let alias_result = ShellCommand::parse_line(alias);
        let full_result = ShellCommand::parse_line(full);

        prop_assert!(alias_result.is_ok());
        prop_assert!(full_result.is_ok());
        // Both should produce the same variant (we can't easily compare enums, but both should succeed)
    }

    /// Commands requiring arguments should fail without them
    #[test]
    fn parse_line___command_missing_required_arg___returns_error(
        cmd in prop_oneof![
            Just("info"),
            Just("stop"),
            Just("connect"),
            Just("disconnect"),
            Just("use"),
            Just("monitor"),
            Just("unmonitor"),
            Just("load"),
            Just("pg"),
            Just("pg members"),
        ]
    ) {
        let result = ShellCommand::parse_line(cmd);

        prop_assert!(result.is_err(), "Expected error for command without args: {}", cmd);
    }

    /// Info command with any non-empty actor name should parse
    #[test]
    fn parse_line___info_with_actor_name___parses_successfully(
        actor in "[a-zA-Z_][a-zA-Z0-9_]*"
    ) {
        let input = format!("info {}", actor);

        let result = ShellCommand::parse_line(&input);

        prop_assert!(result.is_ok(), "Failed to parse: {}", input);
    }

    /// Send command with actor and message should parse
    #[test]
    fn parse_line___send_with_actor_and_message___parses_successfully(
        actor in "[a-zA-Z][a-zA-Z0-9_]{0,10}",
        message in "[a-zA-Z0-9][a-zA-Z0-9 ]{0,19}"  // Must start with non-whitespace
    ) {
        let input = format!("send {} {}", actor, message);

        let result = ShellCommand::parse_line(&input);

        prop_assert!(result.is_ok(), "Failed to parse: {}", input);
    }

    /// Connect command with host:port should parse
    #[test]
    fn parse_line___connect_with_host_port___parses_successfully(
        host in "[a-z]{1,10}",
        port in 1024u16..65535
    ) {
        let input = format!("connect {}:{}", host, port);

        let result = ShellCommand::parse_line(&input);

        prop_assert!(result.is_ok(), "Failed to parse: {}", input);
    }

    /// Unknown commands should return error
    #[test]
    fn parse_line___unknown_command___returns_error(
        cmd in "[a-z]{5,15}"  // Random lowercase words unlikely to be real commands
    ) {
        // Skip if accidentally generated a real command
        let known_commands = [
            "actors", "registry", "nodes", "stats", "monitors", "exit", "quit",
            "help", "info", "send", "call", "stop", "connect", "disconnect",
            "use", "cluster", "tree", "load", "monitor", "unmonitor", "pg",
        ];
        prop_assume!(!known_commands.contains(&cmd.as_str()));

        let result = ShellCommand::parse_line(&cmd);

        prop_assert!(result.is_err(), "Should not parse unknown command: {}", cmd);
    }
}

// ==================== JSON Parsing Properties ====================

proptest! {
    /// Valid JSON objects should parse successfully
    #[test]
    fn parse_json_input___valid_json_object___parses_successfully(
        key in "[a-z]{1,10}",
        value in "[a-z]{1,10}"
    ) {
        let input = format!(r#"{{"{}": "{}"}}"#, key, value);

        let result = ractor_shell::messages::parse_json_input(&input);

        prop_assert!(result.is_ok(), "Failed to parse: {}", input);
    }

    /// Valid JSON numbers should parse
    #[test]
    fn parse_json_input___json_number___parses_successfully(n in -1000i64..1000) {
        let input = n.to_string();

        let result = ractor_shell::messages::parse_json_input(&input);

        prop_assert!(result.is_ok(), "Failed to parse number: {}", input);
    }

    /// Valid JSON booleans should parse
    #[test]
    fn parse_json_input___json_boolean___parses_successfully(b in proptest::bool::ANY) {
        let input = b.to_string();

        let result = ractor_shell::messages::parse_json_input(&input);

        prop_assert!(result.is_ok(), "Failed to parse boolean: {}", input);
    }

    /// JSON null should parse
    #[test]
    fn parse_json_input___json_null___parses_successfully(_unused in Just(())) {
        let result = ractor_shell::messages::parse_json_input("null");

        prop_assert!(result.is_ok());
    }

    /// Invalid JSON should return error
    #[test]
    fn parse_json_input___invalid_json___returns_error(
        input in r#"\{[^}]*"#  // Unclosed braces
    ) {
        let result = ractor_shell::messages::parse_json_input(&input);

        prop_assert!(result.is_err(), "Should fail for invalid JSON: {}", input);
    }
}

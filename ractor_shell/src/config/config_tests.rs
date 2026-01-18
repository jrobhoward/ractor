#![allow(non_snake_case)]

use super::*;

#[test]
fn ShellConfig___default___returns_none_values() {
    let config = ShellConfig::default();

    assert!(config.rpc_timeout_secs.is_none());
    assert!(config.node_server_port.is_none());
    assert!(config.cluster_cookie.is_none());
    assert!(config.auto_connect.is_none());
}

#[test]
fn ShellConfig___rpc_timeout___uses_default_when_not_set() {
    let config = ShellConfig::default();

    let timeout = config.rpc_timeout();

    assert_eq!(timeout, Duration::from_secs(5));
}

#[test]
fn ShellConfig___rpc_timeout___uses_configured_value() {
    let config = ShellConfig {
        rpc_timeout_secs: Some(10),
        ..Default::default()
    };

    let timeout = config.rpc_timeout();

    assert_eq!(timeout, Duration::from_secs(10));
}

#[test]
fn ShellConfig___get_node_server_port___uses_default_when_not_set() {
    let config = ShellConfig::default();

    let port = config.get_node_server_port();

    assert_eq!(port, 9100); // Default port for local NodeServer
}

#[test]
fn ShellConfig___get_node_server_port___uses_configured_value() {
    let config = ShellConfig {
        node_server_port: Some(8080),
        ..Default::default()
    };

    let port = config.get_node_server_port();

    assert_eq!(port, 8080);
}

#[test]
fn ShellConfig___get_cluster_cookie___uses_default_when_not_set() {
    let config = ShellConfig::default();

    let cookie = config.get_cluster_cookie();

    assert_eq!(cookie, "secret_cookie"); // Default cluster cookie
}

#[test]
fn ShellConfig___get_cluster_cookie___uses_configured_value() {
    let config = ShellConfig {
        cluster_cookie: Some("my_cookie".to_string()),
        ..Default::default()
    };

    let cookie = config.get_cluster_cookie();

    assert_eq!(cookie, "my_cookie");
}

#[test]
fn ShellConfig___parse_toml___parses_valid_config() {
    let toml_content = r#"
        rpc_timeout_secs = 15
        node_server_port = 9200
        cluster_cookie = "test_cookie"
        auto_connect = "localhost:9000"
        max_history = 500
    "#;

    let config: ShellConfig = toml::from_str(toml_content).unwrap();

    assert_eq!(config.rpc_timeout_secs, Some(15));
    assert_eq!(config.node_server_port, Some(9200));
    assert_eq!(config.cluster_cookie, Some("test_cookie".to_string()));
    assert_eq!(config.auto_connect, Some("localhost:9000".to_string()));
    assert_eq!(config.max_history, Some(500));
}

#[test]
fn ShellConfig___parse_toml___handles_partial_config() {
    let toml_content = r#"
        rpc_timeout_secs = 20
    "#;

    let config: ShellConfig = toml::from_str(toml_content).unwrap();

    assert_eq!(config.rpc_timeout_secs, Some(20));
    assert!(config.node_server_port.is_none());
    assert!(config.cluster_cookie.is_none());
}

#[test]
fn ShellConfig___get_max_history___uses_default_when_not_set() {
    let config = ShellConfig::default();

    let max = config.get_max_history();

    assert_eq!(max, 1000);
}

#[test]
fn ShellConfig___sample_config___is_valid_toml() {
    let sample = ShellConfig::sample_config();

    let result: Result<ShellConfig, _> = toml::from_str(sample);

    assert!(result.is_ok(), "Sample config should be valid TOML");
}

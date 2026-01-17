//! Tests for Raft leader election

use super::*;

#[test]
fn raft_config___default___has_reasonable_values() {
    let config = RaftConfig::default();

    // Longer timeouts for network latency
    assert_eq!(config.election_timeout_min_ms, 1000);
    assert_eq!(config.election_timeout_max_ms, 2000);
    assert_eq!(config.heartbeat_interval_ms, 300);
    assert!(config.heartbeat_interval_ms < config.election_timeout_min_ms);
}

#[test]
fn raft_state___new___starts_as_follower() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);

    assert_eq!(state.role, RaftRole::Follower);
    assert_eq!(state.current_term, 0);
    assert!(state.voted_for.is_none());
    assert!(state.current_leader.is_none());
    assert!(!state.is_leader());
    assert_eq!(state.peer_count(), 0);
    assert_eq!(state.election_timer_generation, 0);
    assert_eq!(state.heartbeat_timer_generation, 0);
    assert_eq!(state.discovery_timer_generation, 0);
}

#[test]
fn raft_role___display___formats_correctly() {
    assert_eq!(RaftRole::Follower.to_string(), "Follower");
    assert_eq!(RaftRole::Candidate.to_string(), "Candidate");
    assert_eq!(RaftRole::Leader.to_string(), "Leader");
}

#[test]
fn raft_state___election_timeout___returns_value_in_range() {
    let config = RaftConfig {
        node_name: "test".to_string(),
        election_timeout_min_ms: 100,
        election_timeout_max_ms: 200,
        ..Default::default()
    };
    let state = RaftState::new(config);

    let timeout = state.election_timeout();
    let timeout_ms = timeout.as_millis() as u64;

    assert!(
        timeout_ms >= 100 && timeout_ms < 200,
        "timeout {} should be in range [100, 200)",
        timeout_ms
    );
}

#[test]
fn raft_state___heartbeat_interval___returns_configured_value() {
    let config = RaftConfig {
        node_name: "test".to_string(),
        heartbeat_interval_ms: 75,
        ..Default::default()
    };
    let state = RaftState::new(config);

    assert_eq!(state.heartbeat_interval(), Duration::from_millis(75));
}

#[test]
fn raft_state___generation_counters___increments_correctly() {
    let config = RaftConfig {
        node_name: "test".to_string(),
        ..Default::default()
    };
    let mut state = RaftState::new(config);

    assert_eq!(state.next_election_generation(), 1);
    assert_eq!(state.next_election_generation(), 2);
    assert_eq!(state.election_timer_generation, 2);

    assert_eq!(state.next_heartbeat_generation(), 1);
    assert_eq!(state.heartbeat_timer_generation, 1);

    assert_eq!(state.next_discovery_generation(), 1);
    assert_eq!(state.discovery_timer_generation, 1);
}

#[test]
fn vote_response___serialize___produces_valid_json() {
    let response = VoteResponse {
        term: 5,
        vote_granted: true,
        voter_name: "node1".to_string(),
    };

    let json = serde_json::to_string(&response).unwrap();

    assert!(json.contains("\"term\":5"));
    assert!(json.contains("\"vote_granted\":true"));
    assert!(json.contains("\"voter_name\":\"node1\""));
}

#[test]
fn raft_protocol_serialize___election_timeout___produces_valid_json() {
    let msg = RaftProtocol::ElectionTimeout { generation: 5 };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"raft_type\":\"ElectionTimeout\""));
    assert!(json.contains("\"generation\":5"));
}

#[test]
fn raft_protocol_serialize___request_vote___produces_valid_json() {
    let msg = RaftProtocol::RequestVote {
        term: 3,
        candidate_name: "node_a".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"raft_type\":\"RequestVote\""));
    assert!(json.contains("\"term\":3"));
    assert!(json.contains("\"candidate_name\":\"node_a\""));
}

#[test]
fn raft_protocol_serialize___heartbeat___produces_valid_json() {
    let msg = RaftProtocol::Heartbeat {
        term: 5,
        leader_name: "node_b".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"raft_type\":\"Heartbeat\""));
    assert!(json.contains("\"term\":5"));
    assert!(json.contains("\"leader_name\":\"node_b\""));
}

#[test]
fn raft_protocol_deserialize___roundtrip___preserves_values() {
    let original = RaftProtocol::VoteResponse {
        term: 7,
        vote_granted: true,
        voter_name: "node_c".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: RaftProtocol = serde_json::from_str(&json).unwrap();

    match deserialized {
        RaftProtocol::VoteResponse {
            term,
            vote_granted,
            voter_name,
        } => {
            assert_eq!(term, 7);
            assert!(vote_granted);
            assert_eq!(voter_name, "node_c");
        }
        _ => panic!("Expected VoteResponse"),
    }
}

#[test]
fn raft_node_handle_call_command___is_leader___returns_status() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);
    let node = RaftNode;

    let json = serde_json::json!({"command": "is_leader"});
    let response = node.handle_call_command(&state, json);

    match response {
        CallResponse::Success(value) => {
            assert_eq!(value["is_leader"], false);
            assert_eq!(value["node_name"], "test_node");
            assert_eq!(value["term"], 0);
        }
        CallResponse::Error(e) => panic!("Expected success, got error: {}", e),
    }
}

#[test]
fn raft_node_handle_call_command___get_leader___returns_none_initially() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);
    let node = RaftNode;

    let json = serde_json::json!({"command": "get_leader"});
    let response = node.handle_call_command(&state, json);

    match response {
        CallResponse::Success(value) => {
            assert!(value["leader"].is_null());
            assert_eq!(value["term"], 0);
        }
        CallResponse::Error(e) => panic!("Expected success, got error: {}", e),
    }
}

#[test]
fn raft_node_handle_call_command___status___returns_full_status() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);
    let node = RaftNode;

    let json = serde_json::json!({"command": "status"});
    let response = node.handle_call_command(&state, json);

    match response {
        CallResponse::Success(value) => {
            assert_eq!(value["node_name"], "test_node");
            assert_eq!(value["role"], "Follower");
            assert_eq!(value["term"], 0);
            assert!(value["leader"].is_null());
            assert_eq!(value["peers"], 0);
        }
        CallResponse::Error(e) => panic!("Expected success, got error: {}", e),
    }
}

#[test]
fn raft_node_handle_call_command___peers___returns_empty_initially() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);
    let node = RaftNode;

    let json = serde_json::json!({"command": "peers"});
    let response = node.handle_call_command(&state, json);

    match response {
        CallResponse::Success(value) => {
            assert_eq!(value["count"], 0);
            assert!(value["peers"].as_array().unwrap().is_empty());
        }
        CallResponse::Error(e) => panic!("Expected success, got error: {}", e),
    }
}

#[test]
fn raft_node_handle_call_command___unknown___returns_error() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);
    let node = RaftNode;

    let json = serde_json::json!({"command": "invalid_command"});
    let response = node.handle_call_command(&state, json);

    match response {
        CallResponse::Success(_) => panic!("Expected error for unknown command"),
        CallResponse::Error(e) => {
            assert!(e.contains("Unknown command"));
            assert!(e.contains("invalid_command"));
        }
    }
}

#[test]
fn raft_node_handle_call_command___no_command___defaults_to_status() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);
    let node = RaftNode;

    let json = serde_json::json!({});
    let response = node.handle_call_command(&state, json);

    match response {
        CallResponse::Success(value) => {
            // Default is "status" command
            assert_eq!(value["node_name"], "test_node");
            assert_eq!(value["role"], "Follower");
        }
        CallResponse::Error(e) => panic!("Expected success, got error: {}", e),
    }
}

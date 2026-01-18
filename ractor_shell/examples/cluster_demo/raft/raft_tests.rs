//! Tests for Raft leader election
#![allow(non_snake_case)]

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
fn raft_state___get_status___returns_correct_values() {
    let config = RaftConfig {
        node_name: "test_node".to_string(),
        ..Default::default()
    };
    let state = RaftState::new(config);

    let status = state.get_status();

    assert_eq!(status.node_name, "test_node");
    assert_eq!(status.role, "Follower");
    assert_eq!(status.term, 0);
    assert!(status.leader.is_none());
    assert_eq!(status.peers, 0);
    assert!(status.voted_for.is_none());
}

#[test]
fn raft_status___serialize___produces_valid_json() {
    let status = RaftStatus {
        node_name: "node1".to_string(),
        role: "Leader".to_string(),
        term: 5,
        leader: Some("node1".to_string()),
        peers: 2,
        voted_for: Some("node1".to_string()),
    };

    let json = serde_json::to_string(&status).unwrap();

    assert!(json.contains("\"node_name\":\"node1\""));
    assert!(json.contains("\"role\":\"Leader\""));
    assert!(json.contains("\"term\":5"));
    assert!(json.contains("\"peers\":2"));
}

// ==================== RaftMessage SchemaProvider Tests ====================

#[test]
fn raft_message_schema___message_schema___returns_valid_json() {
    use ractor::SchemaProvider;

    let schema = RaftMessage::message_schema();
    let parsed: serde_json::Value =
        serde_json::from_str(schema).expect("Schema should be valid JSON");

    // Verify all variants are present
    let variants = parsed.get("variants").expect("Should have variants");
    assert!(variants.get("ElectionTimeout").is_some());
    assert!(variants.get("HeartbeatTimeout").is_some());
    assert!(variants.get("DiscoverPeers").is_some());
    assert!(variants.get("RequestVote").is_some());
    assert!(variants.get("VoteResponse").is_some());
    assert!(variants.get("Heartbeat").is_some());
    assert!(variants.get("GetStatus").is_some());
    assert!(variants.get("IsLeader").is_some());
    assert!(variants.get("GetLeader").is_some());
    assert!(variants.get("GetPeers").is_some());
}

#[test]
fn raft_message_schema___from_json___parses_request_vote() {
    use ractor::SchemaProvider;

    // Tuple-style variant: RequestVote(term, candidate_name)
    let args = serde_json::json!({
        "0": 5,
        "1": "node_a"
    });
    let msg = RaftMessage::from_json("RequestVote", args).expect("Should parse RequestVote");

    match msg {
        RaftMessage::RequestVote(term, candidate_name) => {
            assert_eq!(term, 5);
            assert_eq!(candidate_name, "node_a");
        }
        _ => panic!("Expected RequestVote variant"),
    }
}

#[test]
fn raft_message_schema___from_json___parses_heartbeat() {
    use ractor::SchemaProvider;

    // Tuple-style variant: Heartbeat(term, leader_name)
    let args = serde_json::json!({
        "0": 10,
        "1": "leader_node"
    });
    let msg = RaftMessage::from_json("Heartbeat", args).expect("Should parse Heartbeat");

    match msg {
        RaftMessage::Heartbeat(term, leader_name) => {
            assert_eq!(term, 10);
            assert_eq!(leader_name, "leader_node");
        }
        _ => panic!("Expected Heartbeat variant"),
    }
}

#[test]
fn raft_message_schema___from_json___parses_vote_response() {
    use ractor::SchemaProvider;

    // Tuple-style variant: VoteResponse(term, vote_granted, voter_name)
    let args = serde_json::json!({
        "0": 3,
        "1": true,
        "2": "voter_node"
    });
    let msg = RaftMessage::from_json("VoteResponse", args).expect("Should parse VoteResponse");

    match msg {
        RaftMessage::VoteResponse(term, vote_granted, voter_name) => {
            assert_eq!(term, 3);
            assert!(vote_granted);
            assert_eq!(voter_name, "voter_node");
        }
        _ => panic!("Expected VoteResponse variant"),
    }
}

#[test]
fn raft_message_schema___from_json___parses_election_timeout() {
    use ractor::SchemaProvider;

    let args = serde_json::json!({"0": 42});
    let msg =
        RaftMessage::from_json("ElectionTimeout", args).expect("Should parse ElectionTimeout");

    match msg {
        RaftMessage::ElectionTimeout(gen) => {
            assert_eq!(gen, 42);
        }
        _ => panic!("Expected ElectionTimeout variant"),
    }
}

#[test]
fn raft_message_schema___from_json___rpc_variant___returns_error() {
    use ractor::SchemaProvider;

    // RPC variants can't be constructed via from_json (they need RpcReplyPort)
    let args = serde_json::json!({});
    let result = RaftMessage::from_json("GetStatus", args);
    assert!(result.is_err(), "RPC variants should return error");
    assert!(
        result.unwrap_err().message.contains("RPC variant"),
        "Error should mention RPC variant"
    );
}

#[test]
fn raft_message_schema___from_json___unknown_variant___returns_error() {
    use ractor::SchemaProvider;

    let args = serde_json::json!({});
    let result = RaftMessage::from_json("UnknownVariant", args);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("unknown variant"));
}

#[test]
fn raft_message_schema___to_json___serializes_request_vote() {
    use ractor::SchemaProvider;

    let msg = RaftMessage::RequestVote(7, "candidate_x".to_string());
    let json = msg.to_json();

    assert_eq!(json.get("variant").unwrap(), "RequestVote");
    assert_eq!(json.get("0").unwrap(), 7);
    assert_eq!(json.get("1").unwrap(), "candidate_x");
}

#[test]
fn raft_message_schema___to_json___serializes_heartbeat() {
    use ractor::SchemaProvider;

    let msg = RaftMessage::Heartbeat(10, "leader_node".to_string());
    let json = msg.to_json();

    assert_eq!(json.get("variant").unwrap(), "Heartbeat");
    assert_eq!(json.get("0").unwrap(), 10);
    assert_eq!(json.get("1").unwrap(), "leader_node");
}

#[test]
fn raft_message_schema___to_json___serializes_vote_response() {
    use ractor::SchemaProvider;

    let msg = RaftMessage::VoteResponse(5, true, "voter_a".to_string());
    let json = msg.to_json();

    assert_eq!(json.get("variant").unwrap(), "VoteResponse");
    assert_eq!(json.get("0").unwrap(), 5);
    assert_eq!(json.get("1").unwrap(), true);
    assert_eq!(json.get("2").unwrap(), "voter_a");
}

//! Tests for ShellProtocolMessage and protocol types
#![allow(non_snake_case)]

use super::*;
use ractor::SchemaProvider;

// ============================================================================
// ShellProtocolMessage SchemaProvider Tests
// ============================================================================

#[test]
fn schema_provider___message_schema___returns_valid_json() {
    let schema = ShellProtocolMessage::message_schema();
    let parsed: serde_json::Value =
        serde_json::from_str(schema).expect("Schema should be valid JSON");
    assert!(
        parsed.get("variants").is_some(),
        "Schema should have variants"
    );

    let variants = parsed.get("variants").unwrap();
    assert!(
        variants.get("StopActor").is_some(),
        "Should have StopActor variant"
    );
    assert!(variants.get("Ping").is_some(), "Should have Ping variant");
}

#[test]
fn schema_provider___from_json_stop_actor___returns_error_as_rpc() {
    // StopActor is now an RPC variant (has RpcReplyPort), so from_json should return error
    let json = serde_json::json!({"0": "test_actor"});
    let result = ShellProtocolMessage::from_json("StopActor", json);
    assert!(result.is_err(), "RPC variants should return error");
    assert!(
        result.unwrap_err().message.contains("RPC variant"),
        "Error should mention RPC variant"
    );
}

#[test]
fn schema_provider___from_json_rpc_variant___returns_error() {
    // RPC variants should return an error (they need RpcReplyPort)
    let json = serde_json::json!({});
    let result = ShellProtocolMessage::from_json("Ping", json);
    assert!(result.is_err(), "RPC variants should return error");
    assert!(
        result.unwrap_err().message.contains("RPC variant"),
        "Error should mention RPC variant"
    );
}

#[test]
fn schema_provider___from_json_unknown_variant___returns_error() {
    let json = serde_json::json!({});
    let result = ShellProtocolMessage::from_json("UnknownVariant", json);
    assert!(result.is_err(), "Unknown variant should return error");
}

#[test]
fn schema_provider___to_json___serializes_cast_variant() {
    // TraceEventNotification is the only non-RPC (cast) variant left
    let event = SerializableTraceEvent {
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        actor_id: Some("0.1".to_string()),
        actor_name: Some("test_actor".to_string()),
        event_type: "Event".to_string(),
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "test message".to_string(),
        fields: vec![],
    };
    let msg = ShellProtocolMessage::TraceEventNotification(event);
    let json = msg.to_json();

    assert_eq!(json.get("variant").unwrap(), "TraceEventNotification");
    // The event is serialized as field "0"
    assert!(json.get("0").is_some(), "Should have event as field 0");
}

// ============================================================================
// DynamicSendResult Tests
// ============================================================================

#[test]
fn DynamicSendResult___debug___formats_all_variants() {
    let success = DynamicSendResult::Success;
    assert!(format!("{:?}", success).contains("Success"));

    let not_found = DynamicSendResult::ActorNotFound;
    assert!(format!("{:?}", not_found).contains("ActorNotFound"));

    let not_dynamic = DynamicSendResult::NotDynamic;
    assert!(format!("{:?}", not_dynamic).contains("NotDynamic"));

    let send_failed = DynamicSendResult::SendFailed("channel closed".to_string());
    assert!(format!("{:?}", send_failed).contains("SendFailed"));
    assert!(format!("{:?}", send_failed).contains("channel closed"));
}

#[test]
fn DynamicSendResult___clone___all_variants___clones_correctly() {
    let success = DynamicSendResult::Success;
    let _cloned = success.clone();

    let send_failed = DynamicSendResult::SendFailed("error".to_string());
    let cloned_failed = send_failed.clone();
    assert!(format!("{:?}", cloned_failed).contains("error"));
}

#[test]
fn DynamicSendResult___serde___round_trip() {
    let variants = vec![
        DynamicSendResult::Success,
        DynamicSendResult::ActorNotFound,
        DynamicSendResult::NotDynamic,
        DynamicSendResult::SendFailed("test error".to_string()),
    ];

    for variant in variants {
        let serialized = serde_json::to_string(&variant).expect("Should serialize");
        let deserialized: DynamicSendResult =
            serde_json::from_str(&serialized).expect("Should deserialize");
        assert_eq!(format!("{:?}", variant), format!("{:?}", deserialized));
    }
}

// ============================================================================
// DynamicCallResult Tests
// ============================================================================

#[test]
fn DynamicCallResult___debug___formats_all_variants() {
    let success = DynamicCallResult::Success(serde_json::json!({"key": "value"}));
    assert!(format!("{:?}", success).contains("Success"));

    let error = DynamicCallResult::Error("actor error".to_string());
    assert!(format!("{:?}", error).contains("Error"));

    let not_found = DynamicCallResult::ActorNotFound;
    assert!(format!("{:?}", not_found).contains("ActorNotFound"));

    let not_dynamic = DynamicCallResult::NotDynamic;
    assert!(format!("{:?}", not_dynamic).contains("NotDynamic"));

    let call_failed = DynamicCallResult::CallFailed("timeout".to_string());
    assert!(format!("{:?}", call_failed).contains("CallFailed"));
}

#[test]
fn DynamicCallResult___clone___all_variants___clones_correctly() {
    let success = DynamicCallResult::Success(serde_json::json!({"data": 42}));
    let cloned = success.clone();
    if let DynamicCallResult::Success(value) = cloned {
        assert_eq!(value.get("data").unwrap(), 42);
    } else {
        panic!("Clone should preserve variant");
    }
}

#[test]
fn DynamicCallResult___serde___round_trip() {
    let variants = vec![
        DynamicCallResult::Success(serde_json::json!({"result": true})),
        DynamicCallResult::Error("error message".to_string()),
        DynamicCallResult::ActorNotFound,
        DynamicCallResult::NotDynamic,
        DynamicCallResult::CallFailed("timeout".to_string()),
    ];

    for variant in variants {
        let serialized = serde_json::to_string(&variant).expect("Should serialize");
        let deserialized: DynamicCallResult =
            serde_json::from_str(&serialized).expect("Should deserialize");
        assert_eq!(format!("{:?}", variant), format!("{:?}", deserialized));
    }
}

// ============================================================================
// TypedRpcResult Tests
// ============================================================================

#[test]
fn TypedRpcResult___debug___formats_all_variants() {
    let success = TypedRpcResult::Success(serde_json::json!({"value": 123}));
    assert!(format!("{:?}", success).contains("Success"));

    let not_found = TypedRpcResult::ActorNotFound;
    assert!(format!("{:?}", not_found).contains("ActorNotFound"));

    let not_schema = TypedRpcResult::NotSchemaEnabled;
    assert!(format!("{:?}", not_schema).contains("NotSchemaEnabled"));

    let unknown = TypedRpcResult::UnknownVariant("BadVariant".to_string());
    assert!(format!("{:?}", unknown).contains("UnknownVariant"));

    let failed = TypedRpcResult::CallFailed("rpc error".to_string());
    assert!(format!("{:?}", failed).contains("CallFailed"));
}

#[test]
fn TypedRpcResult___clone___all_variants___clones_correctly() {
    let success = TypedRpcResult::Success(serde_json::json!({"count": 5}));
    let cloned = success.clone();
    if let TypedRpcResult::Success(value) = cloned {
        assert_eq!(value.get("count").unwrap(), 5);
    } else {
        panic!("Clone should preserve variant");
    }

    let unknown = TypedRpcResult::UnknownVariant("Test".to_string());
    let cloned_unknown = unknown.clone();
    if let TypedRpcResult::UnknownVariant(name) = cloned_unknown {
        assert_eq!(name, "Test");
    } else {
        panic!("Clone should preserve variant");
    }
}

#[test]
fn TypedRpcResult___serde___round_trip() {
    let variants = vec![
        TypedRpcResult::Success(serde_json::json!({"result": "ok"})),
        TypedRpcResult::ActorNotFound,
        TypedRpcResult::NotSchemaEnabled,
        TypedRpcResult::UnknownVariant("GetFoo".to_string()),
        TypedRpcResult::CallFailed("network error".to_string()),
    ];

    for variant in variants {
        let serialized = serde_json::to_string(&variant).expect("Should serialize");
        let deserialized: TypedRpcResult =
            serde_json::from_str(&serialized).expect("Should deserialize");
        assert_eq!(format!("{:?}", variant), format!("{:?}", deserialized));
    }
}

// ============================================================================
// TraceEventBatch Tests
// ============================================================================

#[test]
fn TraceEventBatch___new___with_events_and_dropped() {
    let batch = TraceEventBatch {
        events: vec![SerializableTraceEvent {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            actor_id: Some("0.1".to_string()),
            actor_name: Some("test".to_string()),
            event_type: "Event".to_string(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "hello".to_string(),
            fields: vec![],
        }],
        dropped_count: 5,
    };

    assert_eq!(batch.events.len(), 1);
    assert_eq!(batch.dropped_count, 5);
}

#[test]
fn TraceEventBatch___serde___round_trip() {
    let batch = TraceEventBatch {
        events: vec![],
        dropped_count: 10,
    };

    let serialized = serde_json::to_string(&batch).expect("Should serialize");
    let deserialized: TraceEventBatch =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.dropped_count, 10);
}

// ============================================================================
// SerializableTraceEvent Tests
// ============================================================================

#[test]
fn SerializableTraceEvent___debug___formats_correctly() {
    let event = SerializableTraceEvent {
        timestamp: "2024-01-01T12:00:00Z".to_string(),
        actor_id: Some("0.42".to_string()),
        actor_name: Some("my_actor".to_string()),
        event_type: "ENTER".to_string(),
        level: "DEBUG".to_string(),
        target: "my_module".to_string(),
        message: "entering span".to_string(),
        fields: vec![("key".to_string(), "value".to_string())],
    };

    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("my_actor"));
    assert!(debug_str.contains("ENTER"));
}

#[test]
fn SerializableTraceEvent___clone___preserves_all_fields() {
    let event = SerializableTraceEvent {
        timestamp: "2024-01-01T12:00:00Z".to_string(),
        actor_id: Some("0.1".to_string()),
        actor_name: None,
        event_type: "Event".to_string(),
        level: "WARN".to_string(),
        target: "target".to_string(),
        message: "warning message".to_string(),
        fields: vec![("field1".to_string(), "value1".to_string())],
    };

    let cloned = event.clone();
    assert_eq!(cloned.timestamp, event.timestamp);
    assert_eq!(cloned.actor_id, event.actor_id);
    assert_eq!(cloned.actor_name, event.actor_name);
    assert_eq!(cloned.message, event.message);
    assert_eq!(cloned.fields.len(), 1);
}

#[test]
fn SerializableTraceEvent___serde___round_trip() {
    let event = SerializableTraceEvent {
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        actor_id: Some("0.5".to_string()),
        actor_name: Some("test_actor".to_string()),
        event_type: "EXIT".to_string(),
        level: "TRACE".to_string(),
        target: "module::path".to_string(),
        message: "exiting".to_string(),
        fields: vec![],
    };

    let serialized = serde_json::to_string(&event).expect("Should serialize");
    let deserialized: SerializableTraceEvent =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.actor_name, Some("test_actor".to_string()));
    assert_eq!(deserialized.event_type, "EXIT");
}

// ============================================================================
// MonitorEventBatch Tests
// ============================================================================

#[test]
fn MonitorEventBatch___new___with_events_and_dropped() {
    let batch = MonitorEventBatch {
        events: vec![SerializableMonitorEvent {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            actor_id: "0.1".to_string(),
            actor_name: Some("test".to_string()),
            event_type: "STARTED".to_string(),
            reason: None,
            error: None,
        }],
        dropped_count: 3,
    };

    assert_eq!(batch.events.len(), 1);
    assert_eq!(batch.dropped_count, 3);
}

#[test]
fn MonitorEventBatch___serde___round_trip() {
    let batch = MonitorEventBatch {
        events: vec![],
        dropped_count: 7,
    };

    let serialized = serde_json::to_string(&batch).expect("Should serialize");
    let deserialized: MonitorEventBatch =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.dropped_count, 7);
}

// ============================================================================
// SerializableMonitorEvent Tests
// ============================================================================

#[test]
fn SerializableMonitorEvent___debug___formats_correctly() {
    let event = SerializableMonitorEvent {
        timestamp: "2024-01-01T12:00:00Z".to_string(),
        actor_id: "0.99".to_string(),
        actor_name: Some("monitored_actor".to_string()),
        event_type: "STOPPED".to_string(),
        reason: Some("Normal shutdown".to_string()),
        error: None,
    };

    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("monitored_actor"));
    assert!(debug_str.contains("STOPPED"));
}

#[test]
fn SerializableMonitorEvent___clone___preserves_all_fields() {
    let event = SerializableMonitorEvent {
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        actor_id: "0.1".to_string(),
        actor_name: Some("actor".to_string()),
        event_type: "PANICKED".to_string(),
        reason: None,
        error: Some("panic at line 42".to_string()),
    };

    let cloned = event.clone();
    assert_eq!(cloned.event_type, "PANICKED");
    assert_eq!(cloned.error, Some("panic at line 42".to_string()));
}

#[test]
fn SerializableMonitorEvent___serde___round_trip() {
    let event = SerializableMonitorEvent {
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        actor_id: "0.7".to_string(),
        actor_name: None,
        event_type: "KILLED".to_string(),
        reason: None,
        error: None,
    };

    let serialized = serde_json::to_string(&event).expect("Should serialize");
    let deserialized: SerializableMonitorEvent =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.event_type, "KILLED");
    assert!(deserialized.actor_name.is_none());
}

// ============================================================================
// ActorInfo Tests
// ============================================================================

#[test]
fn ActorInfo___debug___formats_correctly() {
    let info = ActorInfo {
        id: "0.42".to_string(),
        name: Some("test_actor".to_string()),
        status: "Running".to_string(),
        is_local: true,
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("test_actor"));
    assert!(debug_str.contains("Running"));
}

#[test]
fn ActorInfo___clone___preserves_all_fields() {
    let info = ActorInfo {
        id: "0.1".to_string(),
        name: None,
        status: "Stopped".to_string(),
        is_local: false,
    };

    let cloned = info.clone();
    assert_eq!(cloned.id, "0.1");
    assert!(cloned.name.is_none());
    assert_eq!(cloned.status, "Stopped");
    assert!(!cloned.is_local);
}

#[test]
fn ActorInfo___serde___round_trip() {
    let info = ActorInfo {
        id: "1.5".to_string(),
        name: Some("remote_actor".to_string()),
        status: "Running".to_string(),
        is_local: false,
    };

    let serialized = serde_json::to_string(&info).expect("Should serialize");
    let deserialized: ActorInfo = serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.name, Some("remote_actor".to_string()));
    assert!(!deserialized.is_local);
}

// ============================================================================
// SupervisionTreeNode Tests
// ============================================================================

#[test]
fn SupervisionTreeNode___debug___formats_correctly() {
    let node = SupervisionTreeNode {
        actor: ActorInfo {
            id: "0.1".to_string(),
            name: Some("parent".to_string()),
            status: "Running".to_string(),
            is_local: true,
        },
        child_count: 2,
        children: vec![],
    };

    let debug_str = format!("{:?}", node);
    assert!(debug_str.contains("parent"));
    assert!(debug_str.contains("child_count: 2"));
}

#[test]
fn SupervisionTreeNode___clone___preserves_tree_structure() {
    let node = SupervisionTreeNode {
        actor: ActorInfo {
            id: "0.1".to_string(),
            name: Some("root".to_string()),
            status: "Running".to_string(),
            is_local: true,
        },
        child_count: 1,
        children: vec![SupervisionTreeNode {
            actor: ActorInfo {
                id: "0.2".to_string(),
                name: Some("child".to_string()),
                status: "Running".to_string(),
                is_local: true,
            },
            child_count: 0,
            children: vec![],
        }],
    };

    let cloned = node.clone();
    assert_eq!(cloned.children.len(), 1);
    assert_eq!(cloned.children[0].actor.name, Some("child".to_string()));
}

#[test]
fn SupervisionTreeNode___serde___round_trip() {
    let node = SupervisionTreeNode {
        actor: ActorInfo {
            id: "0.1".to_string(),
            name: Some("supervisor".to_string()),
            status: "Running".to_string(),
            is_local: true,
        },
        child_count: 0,
        children: vec![],
    };

    let serialized = serde_json::to_string(&node).expect("Should serialize");
    let deserialized: SupervisionTreeNode =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.actor.name, Some("supervisor".to_string()));
}

// ============================================================================
// ClusterTopology Tests
// ============================================================================

#[test]
fn ClusterTopology___debug___formats_correctly() {
    let topology = ClusterTopology {
        nodes: vec![NodeInfo {
            id: "local".to_string(),
            name: "node_a".to_string(),
            actor_count: 10,
            is_local: true,
        }],
        process_groups: HashMap::new(),
    };

    let debug_str = format!("{:?}", topology);
    assert!(debug_str.contains("node_a"));
}

#[test]
fn ClusterTopology___clone___preserves_all_data() {
    let mut process_groups = HashMap::new();
    process_groups.insert(
        "test_group".to_string(),
        vec![ActorLocation {
            actor_id: "0.1".to_string(),
            actor_name: Some("actor1".to_string()),
            node_id: "local".to_string(),
            node_name: "node_a".to_string(),
        }],
    );

    let topology = ClusterTopology {
        nodes: vec![],
        process_groups,
    };

    let cloned = topology.clone();
    assert!(cloned.process_groups.contains_key("test_group"));
}

#[test]
fn ClusterTopology___serde___round_trip() {
    let topology = ClusterTopology {
        nodes: vec![NodeInfo {
            id: "0".to_string(),
            name: "test_node".to_string(),
            actor_count: 5,
            is_local: true,
        }],
        process_groups: HashMap::new(),
    };

    let serialized = serde_json::to_string(&topology).expect("Should serialize");
    let deserialized: ClusterTopology =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.nodes.len(), 1);
    assert_eq!(deserialized.nodes[0].name, "test_node");
}

// ============================================================================
// NodeInfo Tests
// ============================================================================

#[test]
fn NodeInfo___debug___formats_correctly() {
    let info = NodeInfo {
        id: "1".to_string(),
        name: "worker_node".to_string(),
        actor_count: 25,
        is_local: false,
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("worker_node"));
    assert!(debug_str.contains("25"));
}

#[test]
fn NodeInfo___clone___preserves_all_fields() {
    let info = NodeInfo {
        id: "local".to_string(),
        name: "my_node".to_string(),
        actor_count: 100,
        is_local: true,
    };

    let cloned = info.clone();
    assert_eq!(cloned.id, "local");
    assert_eq!(cloned.actor_count, 100);
    assert!(cloned.is_local);
}

#[test]
fn NodeInfo___serde___round_trip() {
    let info = NodeInfo {
        id: "2".to_string(),
        name: "remote_node".to_string(),
        actor_count: 50,
        is_local: false,
    };

    let serialized = serde_json::to_string(&info).expect("Should serialize");
    let deserialized: NodeInfo = serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.name, "remote_node");
    assert!(!deserialized.is_local);
}

// ============================================================================
// ActorLocation Tests
// ============================================================================

#[test]
fn ActorLocation___debug___formats_correctly() {
    let loc = ActorLocation {
        actor_id: "1.42".to_string(),
        actor_name: Some("remote_worker".to_string()),
        node_id: "1".to_string(),
        node_name: "node_b".to_string(),
    };

    let debug_str = format!("{:?}", loc);
    assert!(debug_str.contains("remote_worker"));
    assert!(debug_str.contains("node_b"));
}

#[test]
fn ActorLocation___clone___preserves_all_fields() {
    let loc = ActorLocation {
        actor_id: "0.5".to_string(),
        actor_name: None,
        node_id: "local".to_string(),
        node_name: "local_node".to_string(),
    };

    let cloned = loc.clone();
    assert_eq!(cloned.actor_id, "0.5");
    assert!(cloned.actor_name.is_none());
}

#[test]
fn ActorLocation___serde___round_trip() {
    let loc = ActorLocation {
        actor_id: "2.10".to_string(),
        actor_name: Some("service".to_string()),
        node_id: "2".to_string(),
        node_name: "node_c".to_string(),
    };

    let serialized = serde_json::to_string(&loc).expect("Should serialize");
    let deserialized: ActorLocation =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.actor_name, Some("service".to_string()));
    assert_eq!(deserialized.node_name, "node_c");
}

// ============================================================================
// SchemaActorInfo Tests
// ============================================================================

#[test]
fn SchemaActorInfo___debug___formats_correctly() {
    let info = SchemaActorInfo {
        name: "schema_actor".to_string(),
        schema: r#"{"type":"object"}"#.to_string(),
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("schema_actor"));
}

#[test]
fn SchemaActorInfo___clone___preserves_all_fields() {
    let info = SchemaActorInfo {
        name: "my_actor".to_string(),
        schema: "schema_content".to_string(),
    };

    let cloned = info.clone();
    assert_eq!(cloned.name, "my_actor");
    assert_eq!(cloned.schema, "schema_content");
}

#[test]
fn SchemaActorInfo___serde___round_trip() {
    let info = SchemaActorInfo {
        name: "test".to_string(),
        schema: r#"{"variants":{}}"#.to_string(),
    };

    let serialized = serde_json::to_string(&info).expect("Should serialize");
    let deserialized: SchemaActorInfo =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.name, "test");
}

// ============================================================================
// RemoteActorMetrics Tests
// ============================================================================

#[test]
fn RemoteActorMetrics___debug___formats_correctly() {
    let metrics = RemoteActorMetrics {
        id: "0.1".to_string(),
        name: Some("metrics_actor".to_string()),
        status: "Running".to_string(),
        groups: vec!["group1".to_string()],
        uptime_ms: 5000,
        message_count: 100,
        handle_time_ns: 1_000_000,
    };

    let debug_str = format!("{:?}", metrics);
    assert!(debug_str.contains("metrics_actor"));
    assert!(debug_str.contains("100"));
}

#[test]
fn RemoteActorMetrics___clone___preserves_all_fields() {
    let metrics = RemoteActorMetrics {
        id: "0.5".to_string(),
        name: None,
        status: "Stopped".to_string(),
        groups: vec!["g1".to_string(), "g2".to_string()],
        uptime_ms: 10000,
        message_count: 50,
        handle_time_ns: 500_000,
    };

    let cloned = metrics.clone();
    assert_eq!(cloned.groups.len(), 2);
    assert_eq!(cloned.message_count, 50);
}

#[test]
fn RemoteActorMetrics___serde___round_trip() {
    let metrics = RemoteActorMetrics {
        id: "0.7".to_string(),
        name: Some("worker".to_string()),
        status: "Running".to_string(),
        groups: vec![],
        uptime_ms: 60000,
        message_count: 1000,
        handle_time_ns: 10_000_000,
    };

    let serialized = serde_json::to_string(&metrics).expect("Should serialize");
    let deserialized: RemoteActorMetrics =
        serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.message_count, 1000);
}

// ============================================================================
// SystemInfo Tests
// ============================================================================

#[test]
fn SystemInfo___default___has_empty_strings_and_zeros() {
    let info = SystemInfo::default();

    assert!(info.hostname.is_empty());
    assert!(info.exe_name.is_empty());
    assert_eq!(info.pid, 0);
    assert_eq!(info.cpu_percent, 0.0);
    assert_eq!(info.memory_bytes, 0);
}

#[test]
fn SystemInfo___memory_string___formats_bytes() {
    let info = SystemInfo {
        memory_bytes: 1024,
        ..Default::default()
    };
    assert_eq!(info.memory_string(), "1.0 KB");

    let info2 = SystemInfo {
        memory_bytes: 1024 * 1024 * 50,
        ..Default::default()
    };
    assert_eq!(info2.memory_string(), "50.0 MB");

    let info3 = SystemInfo {
        memory_bytes: 1024 * 1024 * 1024 * 2,
        ..Default::default()
    };
    assert_eq!(info3.memory_string(), "2.0 GB");
}

#[test]
fn SystemInfo___total_memory_string___formats_bytes() {
    let info = SystemInfo {
        total_memory_bytes: 1024 * 1024 * 1024 * 16,
        ..Default::default()
    };
    assert_eq!(info.total_memory_string(), "16.0 GB");
}

#[test]
fn SystemInfo___uptime_string___seconds() {
    let info = SystemInfo {
        process_uptime_secs: 45,
        ..Default::default()
    };
    assert_eq!(info.uptime_string(), "45s");
}

#[test]
fn SystemInfo___uptime_string___minutes() {
    let info = SystemInfo {
        process_uptime_secs: 125,
        ..Default::default()
    };
    assert_eq!(info.uptime_string(), "2m 5s");
}

#[test]
fn SystemInfo___uptime_string___hours() {
    let info = SystemInfo {
        process_uptime_secs: 3725,
        ..Default::default()
    };
    assert_eq!(info.uptime_string(), "1h 2m");
}

#[test]
fn SystemInfo___uptime_string___days() {
    let info = SystemInfo {
        process_uptime_secs: 90061,
        ..Default::default()
    };
    assert_eq!(info.uptime_string(), "1d 1h");
}

#[test]
fn SystemInfo___serde___round_trip() {
    let info = SystemInfo {
        hostname: "testhost".to_string(),
        exe_name: "myapp".to_string(),
        pid: 12345,
        cpu_percent: 25.5,
        memory_bytes: 1024 * 1024 * 100,
        process_uptime_secs: 3600,
        thread_count: 8,
        total_memory_bytes: 1024 * 1024 * 1024 * 16,
        ractor_shell_version: "0.1.0".to_string(),
    };

    let serialized = serde_json::to_string(&info).expect("Should serialize");
    let deserialized: SystemInfo = serde_json::from_str(&serialized).expect("Should deserialize");

    assert_eq!(deserialized.hostname, "testhost");
    assert_eq!(deserialized.pid, 12345);
    assert_eq!(deserialized.cpu_percent, 25.5);
}

// ============================================================================
// format_bytes Tests
// ============================================================================

#[test]
fn format_bytes___bytes___formats_as_B() {
    assert_eq!(super::format_bytes(0), "0 B");
    assert_eq!(super::format_bytes(512), "512 B");
    assert_eq!(super::format_bytes(1023), "1023 B");
}

#[test]
fn format_bytes___kilobytes___formats_as_KB() {
    assert_eq!(super::format_bytes(1024), "1.0 KB");
    assert_eq!(super::format_bytes(1536), "1.5 KB");
    assert_eq!(super::format_bytes(10240), "10.0 KB");
}

#[test]
fn format_bytes___megabytes___formats_as_MB() {
    assert_eq!(super::format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(super::format_bytes(1024 * 1024 * 50), "50.0 MB");
    assert_eq!(super::format_bytes(1024 * 1024 * 512), "512.0 MB");
}

#[test]
fn format_bytes___gigabytes___formats_as_GB() {
    assert_eq!(super::format_bytes(1024 * 1024 * 1024), "1.0 GB");
    assert_eq!(super::format_bytes(1024 * 1024 * 1024 * 8), "8.0 GB");
}

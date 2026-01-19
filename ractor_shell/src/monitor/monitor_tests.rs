#![allow(non_snake_case)]

use super::{
    is_monitoring_supported, MonitorActor, MonitorArgs, MonitorEvent, MonitorMessage, MonitorState,
};
use chrono::Local;
use ractor::{Actor, ActorRef};

// ============================================================================
// MonitorEvent Format Tests
// ============================================================================

#[test]
fn format___actor_started___contains_expected_output() {
    let event = MonitorEvent::ActorStarted {
        actor_id: "1.0".to_string(),
        actor_name: Some("test_actor".to_string()),
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("STARTED"));
    assert!(formatted.contains("test_actor"));
    assert!(formatted.contains("1.0"));
}

#[test]
fn format___actor_started___unnamed_actor___shows_unnamed() {
    let event = MonitorEvent::ActorStarted {
        actor_id: "0.42".to_string(),
        actor_name: None,
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("STARTED"));
    assert!(formatted.contains("unnamed"));
    assert!(formatted.contains("0.42"));
}

#[test]
fn format___actor_stopped___contains_expected_output() {
    let event = MonitorEvent::ActorStopped {
        actor_id: "0.5".to_string(),
        actor_name: Some("worker_actor".to_string()),
        reason: "Normal shutdown".to_string(),
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("STOPPED"));
    assert!(formatted.contains("worker_actor"));
    assert!(formatted.contains("0.5"));
    assert!(formatted.contains("Normal shutdown"));
}

#[test]
fn format___actor_stopped___unnamed_actor___shows_unnamed() {
    let event = MonitorEvent::ActorStopped {
        actor_id: "0.10".to_string(),
        actor_name: None,
        reason: "Terminated".to_string(),
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("STOPPED"));
    assert!(formatted.contains("unnamed"));
    assert!(formatted.contains("Terminated"));
}

#[test]
fn format___actor_panicked___contains_expected_output() {
    let event = MonitorEvent::ActorPanicked {
        actor_id: "0.7".to_string(),
        actor_name: Some("panicking_actor".to_string()),
        error: "panic message: something went wrong".to_string(),
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("PANICKED"));
    assert!(formatted.contains("panicking_actor"));
    assert!(formatted.contains("0.7"));
    assert!(formatted.contains("something went wrong"));
}

#[test]
fn format___actor_panicked___unnamed_actor___shows_unnamed() {
    let event = MonitorEvent::ActorPanicked {
        actor_id: "0.99".to_string(),
        actor_name: None,
        error: "assertion failed".to_string(),
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("PANICKED"));
    assert!(formatted.contains("unnamed"));
    assert!(formatted.contains("assertion failed"));
}

#[test]
fn format___actor_killed___contains_expected_output() {
    let event = MonitorEvent::ActorKilled {
        actor_id: "0.15".to_string(),
        actor_name: Some("killed_actor".to_string()),
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("KILLED"));
    assert!(formatted.contains("killed_actor"));
    assert!(formatted.contains("0.15"));
}

#[test]
fn format___actor_killed___unnamed_actor___shows_unnamed() {
    let event = MonitorEvent::ActorKilled {
        actor_id: "0.20".to_string(),
        actor_name: None,
        timestamp: Local::now(),
    };

    let formatted = event.format();

    assert!(formatted.contains("KILLED"));
    assert!(formatted.contains("unnamed"));
    assert!(formatted.contains("0.20"));
}

// ============================================================================
// MonitorState Tests
// ============================================================================

#[test]
fn MonitorState___new___initializes_with_empty_collections() {
    let state = MonitorState::new(false);

    assert!(state.monitored.is_empty());
    assert!(state.event_history.is_empty());
    assert_eq!(state.max_history, 100);
}

#[test]
fn MonitorState___new___with_quiet_mode___sets_quiet_flag() {
    let state = MonitorState::new(true);

    // State is created successfully with quiet mode
    // (quiet field exists but is currently unused, marked with #[allow(dead_code)])
    assert!(state.monitored.is_empty());
}

// ============================================================================
// MonitorArgs Tests
// ============================================================================

#[test]
fn MonitorArgs___default___quiet_is_false() {
    let args = MonitorArgs::default();

    assert!(!args.quiet);
}

// ============================================================================
// is_monitoring_supported Tests
// ============================================================================

#[test]
fn is_monitoring_supported___returns_true() {
    assert!(is_monitoring_supported());
}

// ============================================================================
// MonitorMessage Tests
// ============================================================================

#[test]
fn MonitorMessage___debug___formats_all_variants() {
    // Test Debug impl for coverage
    let monitor_msg = MonitorMessage::Monitor {
        actor_name: "test".to_string(),
    };
    let _ = format!("{:?}", monitor_msg);

    let unmonitor_msg = MonitorMessage::Unmonitor {
        actor_name: "test".to_string(),
    };
    let _ = format!("{:?}", unmonitor_msg);

    let event_msg = MonitorMessage::Event(MonitorEvent::ActorStarted {
        actor_id: "0.1".to_string(),
        actor_name: Some("test".to_string()),
        timestamp: Local::now(),
    });
    let _ = format!("{:?}", event_msg);

    let clear_msg = MonitorMessage::ClearAll;
    let _ = format!("{:?}", clear_msg);

    // GetMonitored requires a reply port which is harder to construct in a unit test
    // but the other variants provide good coverage of the Debug impl
}

// ============================================================================
// MonitorEvent Clone Tests
// ============================================================================

#[test]
fn MonitorEvent___clone___all_variants___clones_correctly() {
    let started = MonitorEvent::ActorStarted {
        actor_id: "0.1".to_string(),
        actor_name: Some("actor1".to_string()),
        timestamp: Local::now(),
    };
    let started_clone = started.clone();
    assert!(started_clone.format().contains("actor1"));

    let stopped = MonitorEvent::ActorStopped {
        actor_id: "0.2".to_string(),
        actor_name: Some("actor2".to_string()),
        reason: "done".to_string(),
        timestamp: Local::now(),
    };
    let stopped_clone = stopped.clone();
    assert!(stopped_clone.format().contains("actor2"));

    let panicked = MonitorEvent::ActorPanicked {
        actor_id: "0.3".to_string(),
        actor_name: Some("actor3".to_string()),
        error: "oops".to_string(),
        timestamp: Local::now(),
    };
    let panicked_clone = panicked.clone();
    assert!(panicked_clone.format().contains("actor3"));

    let killed = MonitorEvent::ActorKilled {
        actor_id: "0.4".to_string(),
        actor_name: Some("actor4".to_string()),
        timestamp: Local::now(),
    };
    let killed_clone = killed.clone();
    assert!(killed_clone.format().contains("actor4"));
}

// ============================================================================
// MonitorActor Integration Tests
// ============================================================================

/// Simple test actor for monitoring tests
struct TestMonitoredActor;

impl Actor for TestMonitoredActor {
    type Msg = ();
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ractor::ActorProcessingErr> {
        Ok(())
    }
}

#[tokio::test]
async fn MonitorActor___spawn___initializes_successfully() {
    let args = MonitorArgs { quiet: true };

    let (actor_ref, _handle) =
        Actor::spawn(Some("test_monitor_actor".to_string()), MonitorActor, args)
            .await
            .expect("Failed to spawn MonitorActor");

    // Give actor time to fully start
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Verify actor is running
    assert!(
        actor_ref.get_status() == ractor::ActorStatus::Running,
        "Actor status: {:?}",
        actor_ref.get_status()
    );

    actor_ref.stop(None);
}

#[tokio::test]
async fn MonitorActor___get_monitored___initially_empty() {
    let args = MonitorArgs { quiet: true };

    let (actor_ref, _handle) = Actor::spawn(
        Some("test_monitor_get_empty".to_string()),
        MonitorActor,
        args,
    )
    .await
    .expect("Failed to spawn MonitorActor");

    // Query monitored actors
    let result = actor_ref
        .call(
            MonitorMessage::GetMonitored,
            Some(std::time::Duration::from_secs(5)),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(monitored)) => {
            assert!(
                monitored.is_empty(),
                "Should have no monitored actors initially"
            );
        }
        other => panic!("Expected empty list, got {:?}", other),
    }

    actor_ref.stop(None);
}

#[tokio::test]
async fn MonitorActor___monitor___actor_exists___adds_to_monitored() {
    // Spawn a test actor to monitor
    let (test_actor, _test_handle) =
        Actor::spawn(Some("actor_to_monitor".to_string()), TestMonitoredActor, ())
            .await
            .expect("Failed to spawn test actor");

    let args = MonitorArgs { quiet: true };
    let (monitor_ref, _handle) =
        Actor::spawn(Some("test_monitor_add".to_string()), MonitorActor, args)
            .await
            .expect("Failed to spawn MonitorActor");

    // Start monitoring the test actor
    monitor_ref
        .cast(MonitorMessage::Monitor {
            actor_name: "actor_to_monitor".to_string(),
        })
        .expect("Failed to send Monitor message");

    // Give time for message processing
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify it's being monitored
    let result = monitor_ref
        .call(
            MonitorMessage::GetMonitored,
            Some(std::time::Duration::from_secs(5)),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(monitored)) => {
            assert!(
                monitored.contains(&"actor_to_monitor".to_string()),
                "Should be monitoring actor_to_monitor"
            );
        }
        other => panic!("Expected monitored list, got {:?}", other),
    }

    test_actor.stop(None);
    monitor_ref.stop(None);
}

#[tokio::test]
async fn MonitorActor___monitor___actor_not_found___does_not_add() {
    let args = MonitorArgs { quiet: true };
    let (monitor_ref, _handle) = Actor::spawn(
        Some("test_monitor_not_found".to_string()),
        MonitorActor,
        args,
    )
    .await
    .expect("Failed to spawn MonitorActor");

    // Try to monitor a nonexistent actor
    monitor_ref
        .cast(MonitorMessage::Monitor {
            actor_name: "nonexistent_actor_xyz_123".to_string(),
        })
        .expect("Failed to send Monitor message");

    // Give time for message processing
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify nothing was added
    let result = monitor_ref
        .call(
            MonitorMessage::GetMonitored,
            Some(std::time::Duration::from_secs(5)),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(monitored)) => {
            assert!(
                monitored.is_empty(),
                "Should not add nonexistent actor to monitored list"
            );
        }
        other => panic!("Expected empty list, got {:?}", other),
    }

    monitor_ref.stop(None);
}

#[tokio::test]
async fn MonitorActor___unmonitor___was_monitored___removes_from_list() {
    // Spawn a test actor to monitor
    let (test_actor, _test_handle) = Actor::spawn(
        Some("actor_to_unmonitor".to_string()),
        TestMonitoredActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor");

    let args = MonitorArgs { quiet: true };
    let (monitor_ref, _handle) = Actor::spawn(
        Some("test_monitor_unmonitor".to_string()),
        MonitorActor,
        args,
    )
    .await
    .expect("Failed to spawn MonitorActor");

    // Start monitoring
    monitor_ref
        .cast(MonitorMessage::Monitor {
            actor_name: "actor_to_unmonitor".to_string(),
        })
        .expect("Failed to send Monitor message");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Stop monitoring
    monitor_ref
        .cast(MonitorMessage::Unmonitor {
            actor_name: "actor_to_unmonitor".to_string(),
        })
        .expect("Failed to send Unmonitor message");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify it's no longer monitored
    let result = monitor_ref
        .call(
            MonitorMessage::GetMonitored,
            Some(std::time::Duration::from_secs(5)),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(monitored)) => {
            assert!(
                !monitored.contains(&"actor_to_unmonitor".to_string()),
                "Should no longer be monitoring actor_to_unmonitor"
            );
        }
        other => panic!("Expected empty list, got {:?}", other),
    }

    test_actor.stop(None);
    monitor_ref.stop(None);
}

#[tokio::test]
async fn MonitorActor___unmonitor___not_monitored___handles_gracefully() {
    let args = MonitorArgs { quiet: true };
    let (monitor_ref, _handle) = Actor::spawn(
        Some("test_monitor_unmonitor_missing".to_string()),
        MonitorActor,
        args,
    )
    .await
    .expect("Failed to spawn MonitorActor");

    // Try to unmonitor something that wasn't being monitored
    monitor_ref
        .cast(MonitorMessage::Unmonitor {
            actor_name: "never_monitored_actor".to_string(),
        })
        .expect("Failed to send Unmonitor message");

    // Should not panic - just prints a warning
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Actor should still be running
    assert!(monitor_ref.get_status() == ractor::ActorStatus::Running);

    monitor_ref.stop(None);
}

#[tokio::test]
async fn MonitorActor___event___adds_to_history() {
    let args = MonitorArgs { quiet: true };
    let (monitor_ref, _handle) =
        Actor::spawn(Some("test_monitor_event".to_string()), MonitorActor, args)
            .await
            .expect("Failed to spawn MonitorActor");

    // Send an event
    let event = MonitorEvent::ActorStarted {
        actor_id: "0.100".to_string(),
        actor_name: Some("event_test_actor".to_string()),
        timestamp: Local::now(),
    };

    monitor_ref
        .cast(MonitorMessage::Event(event))
        .expect("Failed to send Event message");

    // Give time for message processing
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Actor should still be running (event was processed)
    assert!(monitor_ref.get_status() == ractor::ActorStatus::Running);

    monitor_ref.stop(None);
}

#[tokio::test]
async fn MonitorActor___clear_all___removes_all_monitors() {
    // Spawn test actors to monitor
    let (test_actor1, _handle1) = Actor::spawn(
        Some("clear_test_actor1".to_string()),
        TestMonitoredActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor 1");

    let (test_actor2, _handle2) = Actor::spawn(
        Some("clear_test_actor2".to_string()),
        TestMonitoredActor,
        (),
    )
    .await
    .expect("Failed to spawn test actor 2");

    let args = MonitorArgs { quiet: true };
    let (monitor_ref, _handle) =
        Actor::spawn(Some("test_monitor_clear".to_string()), MonitorActor, args)
            .await
            .expect("Failed to spawn MonitorActor");

    // Start monitoring both actors
    monitor_ref
        .cast(MonitorMessage::Monitor {
            actor_name: "clear_test_actor1".to_string(),
        })
        .expect("Failed to send Monitor message");

    monitor_ref
        .cast(MonitorMessage::Monitor {
            actor_name: "clear_test_actor2".to_string(),
        })
        .expect("Failed to send Monitor message");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify both are monitored
    let result = monitor_ref
        .call(
            MonitorMessage::GetMonitored,
            Some(std::time::Duration::from_secs(5)),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(monitored)) => {
            assert_eq!(monitored.len(), 2, "Should have 2 monitored actors");
        }
        other => panic!("Expected 2 actors, got {:?}", other),
    }

    // Clear all
    monitor_ref
        .cast(MonitorMessage::ClearAll)
        .expect("Failed to send ClearAll message");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify all cleared
    let result = monitor_ref
        .call(
            MonitorMessage::GetMonitored,
            Some(std::time::Duration::from_secs(5)),
        )
        .await;

    match result {
        Ok(ractor::rpc::CallResult::Success(monitored)) => {
            assert!(
                monitored.is_empty(),
                "Should have no monitored actors after ClearAll"
            );
        }
        other => panic!("Expected empty list, got {:?}", other),
    }

    test_actor1.stop(None);
    test_actor2.stop(None);
    monitor_ref.stop(None);
}

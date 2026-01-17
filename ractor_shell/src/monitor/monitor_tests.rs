#![allow(non_snake_case)]

use crate::monitor::MonitorEvent;
use chrono::Local;

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
}

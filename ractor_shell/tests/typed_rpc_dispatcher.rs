#![allow(non_snake_case)]

use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort, SchemaProvider};
use ractor_cluster::RactorClusterMessage;
use ractor_shell::schema_registry;
use serde::{Deserialize, Serialize};

/// Test message enum with RPC variants for dispatcher testing.
#[derive(RactorClusterMessage, Debug)]
#[ractor_shell]
pub enum TestMessage {
    #[rpc]
    GetValue(RpcReplyPort<i32>),

    #[rpc]
    GetStatus(RpcReplyPort<TestStatus>),

    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStatus {
    pub active: bool,
    pub count: i32,
}

struct TestActor;

impl Actor for TestActor {
    type Msg = TestMessage;
    type State = i32;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        if let Some(name) = myself.get_name() {
            schema_registry::register::<TestMessage>(&name);
        }
        Ok(42)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            TestMessage::GetValue(reply) => {
                let _ = reply.send(*state);
            }
            TestMessage::GetStatus(reply) => {
                let status = TestStatus {
                    active: true,
                    count: *state,
                };
                let _ = reply.send(status);
            }
            TestMessage::Ping => {}
        }
        Ok(())
    }
}

#[tokio::test]
async fn schema_registry___register_with_rpc_variants___stores_dispatcher() {
    let (actor_ref, _handle) = Actor::spawn(Some("test_dispatcher_1".to_string()), TestActor, ())
        .await
        .unwrap();

    let has_dispatcher = schema_registry::has_dispatcher("test_dispatcher_1");

    assert!(has_dispatcher);
    actor_ref.stop(None);
}

#[tokio::test]
async fn dispatcher___call_get_value___returns_correct_result() {
    let (actor_ref, _handle) = Actor::spawn(Some("test_dispatcher_2".to_string()), TestActor, ())
        .await
        .unwrap();

    let dispatcher = schema_registry::get_dispatcher("test_dispatcher_2").unwrap();
    let result = dispatcher(
        actor_ref.get_cell(),
        "GetValue".to_string(),
        serde_json::json!({}),
    )
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(42));
    actor_ref.stop(None);
}

#[tokio::test]
async fn dispatcher___call_get_status___returns_serialized_struct() {
    let (actor_ref, _handle) = Actor::spawn(Some("test_dispatcher_3".to_string()), TestActor, ())
        .await
        .unwrap();

    let dispatcher = schema_registry::get_dispatcher("test_dispatcher_3").unwrap();
    let result = dispatcher(
        actor_ref.get_cell(),
        "GetStatus".to_string(),
        serde_json::json!({}),
    )
    .await;

    assert!(result.is_ok());
    let status_json = result.unwrap();
    assert_eq!(status_json["active"], true);
    assert_eq!(status_json["count"], 42);
    actor_ref.stop(None);
}

#[tokio::test]
async fn dispatcher___call_unknown_variant___returns_error() {
    let (actor_ref, _handle) = Actor::spawn(Some("test_dispatcher_4".to_string()), TestActor, ())
        .await
        .unwrap();

    let dispatcher = schema_registry::get_dispatcher("test_dispatcher_4").unwrap();
    let result = dispatcher(
        actor_ref.get_cell(),
        "UnknownRpc".to_string(),
        serde_json::json!({}),
    )
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown RPC variant"));
    actor_ref.stop(None);
}

#[tokio::test]
async fn dispatcher___actor_stopped___returns_error() {
    let (actor_ref, _handle) = Actor::spawn(Some("test_dispatcher_5".to_string()), TestActor, ())
        .await
        .unwrap();
    let cell = actor_ref.get_cell();

    actor_ref.stop(None);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let dispatcher = schema_registry::get_dispatcher("test_dispatcher_5").unwrap();
    let result = dispatcher(cell, "GetValue".to_string(), serde_json::json!({})).await;

    // Actor is stopped, so we should get some kind of error
    assert!(result.is_err());
}

#[test]
fn SchemaProvider___dispatcher_method___returns_some_for_rpc_variants() {
    let dispatcher = TestMessage::dispatcher();

    assert!(dispatcher.is_some());
}

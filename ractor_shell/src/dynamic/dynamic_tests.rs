#![allow(non_snake_case)]

use crate::dynamic::{supports_dynamic_messages, CallResponse, DynamicMessage};
use ractor::{Actor, ActorProcessingErr, ActorRef};

struct TestActor;

impl Actor for TestActor {
    type Msg = DynamicMessage;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DynamicMessage::Cast(json) => {
                println!("Received cast: {:?}", json);
                Ok(())
            }
            DynamicMessage::Call(json, reply) => {
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "ping" => {
                            let _ = reply
                                .send(CallResponse::Success(serde_json::json!({"pong": true})));
                        }
                        _ => {
                            let _ = reply
                                .send(CallResponse::Error(format!("Unknown command: {}", cmd)));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error("No command field".to_string()));
                }
                Ok(())
            }
            DynamicMessage::Ping(reply) => {
                let _ = reply.send(true);
                Ok(())
            }
        }
    }
}

#[tokio::test]
async fn supports_dynamic_messages___actor_responds_to_ping___returns_true() {
    let (actor_ref, _handle) = Actor::spawn(None, TestActor, ())
        .await
        .expect("Failed to spawn test actor");

    let supports = supports_dynamic_messages(actor_ref.clone()).await;

    assert!(supports);
    actor_ref.stop(None);
}

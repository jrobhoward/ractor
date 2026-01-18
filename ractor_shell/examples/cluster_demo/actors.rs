//! Demo actors for the cluster demo example.
//!
//! This module contains various actor implementations that demonstrate
//! ractor_shell features:
//!
//! - `DemoActor` - Simple stateless actor for basic demonstrations
//! - `DynamicDemoActor` - Actor supporting dynamic JSON messages from the shell
//! - `PanickyActor` - Actor that can be triggered to panic (for monitoring demos)

use ractor::{Actor, ActorProcessingErr, ActorRef};
use ractor_shell::dynamic::{CallResponse, DynamicMessage};
use serde_json::json;

// ==================== Simple Demo Actor ====================

/// Simple demo actor that responds to basic messages.
pub struct DemoActor;

/// Messages for the simple demo actor.
#[derive(Debug)]
pub enum DemoMessage {
    /// Simple ping message
    Ping,
}

// Note: We don't derive Serialize/Deserialize for local-only messages
// to avoid conflicts with the blanket Message impl under blanket_serde feature.
impl ractor::Message for DemoMessage {}

impl Actor for DemoActor {
    type Msg = DemoMessage;
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
            DemoMessage::Ping => {
                tracing::debug!("DemoActor received Ping");
            }
        }
        Ok(())
    }
}

// ==================== Dynamic Demo Actor ====================

/// Actor that supports dynamic JSON messages from the shell.
///
/// This actor demonstrates how to receive arbitrary JSON messages
/// using the `DynamicMessage` interface. Commands include:
/// - `increment` - Increment the counter
/// - `reset` - Reset the counter to zero
/// - `set_status` - Set a status string
/// - `get_counter` - RPC: Get the current counter value
/// - `get_status` - RPC: Get the full status
/// - `add` - RPC: Add a value to the counter
pub struct DynamicDemoActor;

/// State for the dynamic demo actor.
pub struct DynamicDemoActorState {
    /// A simple counter value
    pub counter: i32,
    /// A status string
    pub status: String,
}

impl Actor for DynamicDemoActor {
    type Msg = DynamicMessage;
    type State = DynamicDemoActorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        tracing::info!("DynamicDemoActor started");
        Ok(DynamicDemoActorState {
            counter: 0,
            status: "idle".to_string(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DynamicMessage::Cast(json) => {
                tracing::debug!(message = %json, "DynamicDemoActor received cast");

                // Handle different message patterns
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "increment" => {
                            state.counter += 1;
                            tracing::debug!(counter = state.counter, "Counter incremented");
                        }
                        "reset" => {
                            state.counter = 0;
                            tracing::debug!("Counter reset");
                        }
                        "set_status" => {
                            if let Some(status) = json.get("status").and_then(|v| v.as_str()) {
                                state.status = status.to_string();
                                tracing::debug!(status = %status, "Status set");
                            }
                        }
                        _ => {
                            tracing::warn!(command = %cmd, "Unknown command");
                        }
                    }
                } else {
                    tracing::debug!("Received unstructured message");
                }
                Ok(())
            }
            DynamicMessage::Call(json, reply) => {
                tracing::debug!(message = %json, "DynamicDemoActor received call");

                // Handle RPC messages
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "get_counter" => {
                            let response = json!({
                                "counter": state.counter
                            });
                            let _ = reply.send(CallResponse::Success(response));
                        }
                        "get_status" => {
                            let response = json!({
                                "status": state.status,
                                "counter": state.counter
                            });
                            let _ = reply.send(CallResponse::Success(response));
                        }
                        "add" => {
                            if let Some(amount) = json.get("amount").and_then(|v| v.as_i64()) {
                                state.counter += amount as i32;
                                let response = json!({
                                    "new_value": state.counter
                                });
                                let _ = reply.send(CallResponse::Success(response));
                            } else {
                                let _ = reply.send(CallResponse::Error(
                                    "Missing or invalid 'amount' field".to_string(),
                                ));
                            }
                        }
                        _ => {
                            let _ = reply
                                .send(CallResponse::Error(format!("Unknown command: {}", cmd)));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error(
                        "No 'command' field in message".to_string(),
                    ));
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

// ==================== Panicky Actor ====================

/// Actor that will panic when triggered.
///
/// This actor is used for demonstrating actor monitoring and supervision.
/// Send `PanickyMessage::Panic` to trigger a panic.
pub struct PanickyActor;

/// Messages for the panicky actor.
#[derive(Debug)]
pub enum PanickyMessage {
    /// Trigger a panic
    Panic,
    /// Normal message (does nothing harmful)
    Normal,
}

// Note: We don't derive Serialize/Deserialize for local-only messages
// to avoid conflicts with the blanket Message impl under blanket_serde feature.
impl ractor::Message for PanickyMessage {}

impl Actor for PanickyActor {
    type Msg = PanickyMessage;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        tracing::info!("PanickyActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            PanickyMessage::Panic => {
                panic!("Intentional panic for monitoring demo");
            }
            PanickyMessage::Normal => {
                tracing::debug!("PanickyActor received Normal message");
            }
        }
        Ok(())
    }
}

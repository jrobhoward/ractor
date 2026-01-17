//! Dynamic Message Interface for Shell Interaction
//!
//! This module provides a way for actors to receive arbitrary JSON messages
//! from the shell while maintaining Rust's type safety.
//!
//! ## Design
//!
//! Actors that want to receive JSON messages from the shell use `DynamicMessage`
//! as their message type. The shell can then send arbitrary JSON to these actors.
//!
//! ## Example
//!
//! ```rust,ignore
//! use ractor_shell::dynamic::{DynamicMessage, CallResponse};
//! use ractor::{Actor, ActorRef, ActorProcessingErr};
//! use serde_json::json;
//!
//! struct MyActor;
//!
//! #[async_trait::async_trait]
//! impl Actor for MyActor {
//!     type Msg = DynamicMessage;
//!     type State = String;
//!     type Arguments = ();
//!
//!     async fn pre_start(&self, _myself: ActorRef<Self::Msg>, _: ())
//!         -> Result<Self::State, ActorProcessingErr> {
//!         Ok("idle".to_string())
//!     }
//!
//!     async fn handle(&self, _myself: ActorRef<Self::Msg>, message: Self::Msg, state: &mut Self::State)
//!         -> Result<(), ActorProcessingErr> {
//!         match message {
//!             DynamicMessage::Cast(json) => {
//!                 println!("Received: {:?}", json);
//!                 *state = json.get("status").and_then(|v| v.as_str())
//!                     .unwrap_or("unknown").to_string();
//!                 Ok(())
//!             }
//!             DynamicMessage::Call(json, reply) => {
//!                 if json.get("command") == Some(&json!("get_status")) {
//!                     let _ = reply.send(CallResponse::Success(json!({"status": state.clone()})));
//!                 } else {
//!                     let _ = reply.send(CallResponse::Error("Unknown command".to_string()));
//!                 }
//!                 Ok(())
//!             }
//!             DynamicMessage::Ping(reply) => {
//!                 let _ = reply.send(true);
//!                 Ok(())
//!             }
//!         }
//!     }
//! }
//! ```

use ractor::{rpc::CallResult, RpcReplyPort};
use serde_json::Value;

/// A message type that can carry arbitrary JSON data
///
/// Actors using this as their message type can receive JSON from the shell.
#[derive(Debug)]
pub enum DynamicMessage {
    /// Cast message (fire-and-forget) with JSON payload
    ///
    /// Sent by the shell when using: `send <actor> <json>`
    Cast(Value),

    /// Call message (RPC) with JSON payload and reply port
    ///
    /// Sent by the shell when using: `call <actor> <json>`
    Call(Value, RpcReplyPort<CallResponse>),

    /// Query to check if the actor supports dynamic messages
    ///
    /// This is used by the shell to determine if it can send JSON messages.
    /// Actors should reply with `true`.
    Ping(RpcReplyPort<bool>),
}

impl ractor::Message for DynamicMessage {}

/// Response from an RPC call
#[derive(Debug, Clone)]
pub enum CallResponse {
    /// Successful response with JSON data
    Success(Value),

    /// Error response with message
    Error(String),
}

/// Helper function to check if an actor supports dynamic messages
///
/// This sends a Ping RPC to the actor and returns true if it responds.
/// The shell uses this to determine if it can send JSON messages to an actor.
pub async fn supports_dynamic_messages(actor_ref: ractor::ActorRef<DynamicMessage>) -> bool {
    match actor_ref
        .call(
            DynamicMessage::Ping,
            Some(tokio::time::Duration::from_millis(500)),
        )
        .await
    {
        Ok(CallResult::Success(result)) => result,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}

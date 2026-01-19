//! Messaging Commands
//!
//! Commands for sending messages to actors:
//! - `send` - Send a cast message (fire-and-forget)
//! - `call` - Send an RPC message and wait for reply
//! - `send-file` - Send message from JSON file

use colored::Colorize;
use ractor::rpc::CallResult;
use ractor::{registry, ActorRef};

use crate::error::{ShellError, ShellResult};
use crate::protocol::{self, ShellProtocolMessage};
use crate::ShellState;
use crate::{messages, schema_registry, DEFAULT_RPC_TIMEOUT};

impl ShellState {
    /// Send a cast message to an actor (fire-and-forget).
    pub(crate) async fn cmd_send(&self, actor: String, message: String) -> ShellResult<()> {
        println!("send {} <- {}", actor.green(), message.bright_black());
        println!();

        // Try to parse the message as JSON
        let json_value = match messages::parse_json_input(&message) {
            Ok(v) => v,
            Err(e) => {
                println!("{} {}", "Parse error:".red().bold(), e);
                println!();
                println!("{}", "Expected JSON format. Examples:".bright_black());
                println!(
                    "{}",
                    r#"  send ping_pong {{"Ping": ["shell", 1]}}"#.bright_black()
                );
                println!(
                    "{}",
                    r#"  send my_actor {{"DoWork": ["task1"]}}"#.bright_black()
                );
                return Ok(());
            }
        };

        // Check if we're in remote or local mode
        if let Some(ref node_name) = self.current_node {
            // Remote send via IntrospectionActor
            return self
                .cmd_send_remote(node_name.clone(), actor, json_value)
                .await;
        }

        // Local send attempt
        // Find the actor in registry
        if let Some(cell) = registry::where_is(actor.clone()) {
            // Try to convert to a dynamic message actor
            let dynamic_ref: ActorRef<crate::dynamic::DynamicMessage> =
                ActorRef::from(cell.clone());

            // Check if the actor supports dynamic messages
            if crate::dynamic::supports_dynamic_messages(dynamic_ref.clone()).await {
                // Actor supports dynamic messages - actually send!
                println!("{} Actor supports dynamic messages", "✓".green());

                dynamic_ref
                    .cast(crate::dynamic::DynamicMessage::Cast(json_value.clone()))
                    .map_err(ShellError::messaging)?;

                println!("{} Message sent to {}", "✓".green().bold(), actor.green());
                println!();
                println!("{}", "Message:".bold());
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_value)
                        .unwrap_or_default()
                        .bright_black()
                );
            } else {
                // Actor doesn't support dynamic messages - show educational message
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
                println!(
                    "  {} See the documentation for implementing dynamic message support.",
                    "•".bright_black()
                );
                println!(
                    "{}",
                    "  Example: use ractor_shell::dynamic::DynamicMessage;".bright_black()
                );
                println!();
                println!(
                    "{}",
                    format!("Actor: {} (ID: {})", actor.green(), cell.get_id()).bright_black()
                );
                println!(
                    "{}",
                    format!(
                        "Message: {}",
                        serde_json::to_string_pretty(&json_value).unwrap_or_default()
                    )
                    .bright_black()
                );
                println!();
                println!("{}", "How to enable:".bold());
                println!(
                    "{}",
                    "  1. Change your actor to use DynamicMessage as its Msg type".bright_black()
                );
                println!(
                    "{}",
                    "  2. Handle DynamicMessage::Cast/Call/Ping in your handler".bright_black()
                );
                println!(
                    "{}",
                    "  3. See examples/dynamic_actor.rs for a complete example".bright_black()
                );
            }
        } else {
            println!("{} Actor '{}' not found in registry", "✗".red(), actor);
        }

        Ok(())
    }

    /// Send an RPC call to an actor and wait for reply.
    pub(crate) async fn cmd_call(&self, actor: String, message: String) -> ShellResult<()> {
        println!("call {} <- {}", actor.green(), message.bright_black());
        println!();

        // Check if the message looks like typed syntax: "VariantName {args}"
        // This pattern: starts with uppercase letter, followed by optional whitespace and JSON object
        let is_typed_syntax = Self::looks_like_typed_message(&message);

        // For remote calls with typed syntax, always try typed RPC
        // (the remote node might have the schema even if we don't locally)
        if is_typed_syntax && self.current_node.is_some() {
            return self.cmd_call_typed(&actor, &message).await;
        }

        // For local calls, check if actor has a registered schema
        if schema_registry::has_schema(&actor) {
            return self.cmd_call_typed(&actor, &message).await;
        }

        // Try to parse the message as JSON for DynamicMessage actors
        let json_value = match messages::parse_json_input(&message) {
            Ok(v) => v,
            Err(e) => {
                println!("{} {}", "Parse error:".red().bold(), e);
                println!();
                println!("{}", "Expected JSON format. Examples:".bright_black());
                println!(
                    "{}",
                    r#"  call ping_pong {{"GetStats": null}}"#.bright_black()
                );
                println!(
                    "{}",
                    r#"  call my_actor {{"QueryData": ["key1"]}}"#.bright_black()
                );
                return Ok(());
            }
        };

        // Check if we're in remote or local mode
        if let Some(ref node_name) = self.current_node {
            // Remote call via IntrospectionActor
            return self
                .cmd_call_remote(node_name.clone(), actor, json_value)
                .await;
        }

        // Local call attempt
        if let Some(cell) = registry::where_is(actor.clone()) {
            // Try to convert to a dynamic message actor
            let dynamic_ref: ActorRef<crate::dynamic::DynamicMessage> =
                ActorRef::from(cell.clone());

            // Check if the actor supports dynamic messages
            if crate::dynamic::supports_dynamic_messages(dynamic_ref.clone()).await {
                // Actor supports dynamic messages - actually call!
                println!("{} Actor supports dynamic messages", "✓".green());
                println!("{} Making RPC call...", "•".bright_black());
                println!();

                // Make the RPC call with a timeout
                let result = dynamic_ref
                    .call(
                        |reply| crate::dynamic::DynamicMessage::Call(json_value.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await;

                match result {
                    Ok(CallResult::Success(response)) => match response {
                        crate::dynamic::CallResponse::Success(value) => {
                            println!("{} RPC call successful", "✓".green().bold());
                            println!();
                            println!("{}", "Response:".bold());
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&value)
                                    .unwrap_or_default()
                                    .green()
                            );
                        }
                        crate::dynamic::CallResponse::Error(err) => {
                            println!("{} RPC call failed", "✗".red().bold());
                            println!();
                            println!("{} {}", "Error:".red(), err);
                        }
                    },
                    Ok(CallResult::Timeout) => {
                        println!("{} RPC call timed out after 5 seconds", "✗".red().bold());
                    }
                    Ok(CallResult::SenderError) => {
                        println!("{} RPC call sender error", "✗".red().bold());
                    }
                    Err(e) => {
                        println!("{} RPC call error: {}", "✗".red().bold(), e);
                    }
                }
            } else {
                // Actor doesn't support dynamic messages - show educational message
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
                println!(
                    "  {} Handle DynamicMessage::Call variant to respond to RPC calls.",
                    "•".bright_black()
                );
                println!();
                println!(
                    "{}",
                    format!("Actor: {} (ID: {})", actor.green(), cell.get_id()).bright_black()
                );
                println!(
                    "{}",
                    format!(
                        "Message: {}",
                        serde_json::to_string_pretty(&json_value).unwrap_or_default()
                    )
                    .bright_black()
                );
                println!();
                println!("{}", "How to enable:".bold());
                println!(
                    "{}",
                    "  1. Change your actor to use DynamicMessage as its Msg type".bright_black()
                );
                println!(
                    "{}",
                    "  2. Handle DynamicMessage::Call and send a CallResponse".bright_black()
                );
                println!(
                    "{}",
                    "  3. See examples/dynamic_actor.rs for a complete example".bright_black()
                );
            }
        } else {
            println!("{} Actor '{}' not found in registry", "✗".red(), actor);
        }

        Ok(())
    }

    /// Call a schema-enabled actor with typed RPC.
    pub(crate) async fn cmd_call_typed(&self, actor: &str, message: &str) -> ShellResult<()> {
        // Parse the message as "VariantName {args}" or "VariantName {}"
        let (variant, args) = Self::parse_typed_message(message)?;

        // Check if we're in remote mode
        if let Some(ref node_name) = self.current_node {
            return self
                .cmd_call_typed_remote(node_name.clone(), actor.to_string(), variant, args)
                .await;
        }

        // Local call - handle known typed actors
        let Some(_cell) = registry::where_is(actor.to_string()) else {
            println!("{} Actor '{}' not found in registry", "✗".red(), actor);
            return Ok(());
        };

        // For schema actors, show helpful message
        println!(
            "{}",
            "⚠ Typed RPC not yet implemented for this actor".yellow()
        );
        println!();
        println!(
            "  {} Actor '{}' has a schema but typed RPC requires explicit support.",
            "•".bright_black(),
            actor
        );
        println!(
            "  {} Schema: {}",
            "•".bright_black(),
            schema_registry::get_schema(actor).unwrap_or_default()
        );

        Ok(())
    }

    /// Check if a message looks like typed syntax: "VariantName {}" or "VariantName {args}".
    /// Returns true if message starts with uppercase letter and contains braces.
    pub(crate) fn looks_like_typed_message(message: &str) -> bool {
        let message = message.trim();
        // Must start with uppercase letter (Rust enum variant convention)
        let starts_with_upper = message
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        // Must contain '{' somewhere
        let has_brace = message.contains('{');
        starts_with_upper && has_brace
    }

    /// Parse a typed message in format "VariantName {args}" or "VariantName {}".
    pub(crate) fn parse_typed_message(message: &str) -> ShellResult<(String, serde_json::Value)> {
        let message = message.trim();

        // Find the variant name (everything before the first '{' or whitespace)
        let variant_end = message
            .find(|c: char| c == '{' || c.is_whitespace())
            .unwrap_or(message.len());

        let variant = message[..variant_end].trim().to_string();

        if variant.is_empty() {
            return Err(ShellError::ParseError(
                "Missing variant name. Use format: VariantName {}".to_string(),
            ));
        }

        // Parse the args (everything from '{' to end, or default to {})
        let args_str = if let Some(brace_pos) = message.find('{') {
            message[brace_pos..].trim()
        } else {
            "{}"
        };

        let args: serde_json::Value = serde_json::from_str(args_str).map_err(|e| {
            ShellError::ParseError(format!(
                "Invalid JSON args: {}. Use format: VariantName {{}}",
                e
            ))
        })?;

        Ok((variant, args))
    }

    /// Call a typed RPC on a remote node.
    pub(crate) async fn cmd_call_typed_remote(
        &self,
        node_name: String,
        actor: String,
        variant: String,
        args: serde_json::Value,
    ) -> ShellResult<()> {
        let introspection_actor = self
            .connected_nodes
            .get(&node_name)
            .ok_or_else(|| ShellError::NodeNotConnected(node_name.clone()))?;

        // Use CallTypedRpc protocol message
        let result = introspection_actor
            .call(
                |reply| {
                    ShellProtocolMessage::CallTypedRpc(
                        actor.clone(),
                        variant.clone(),
                        args.clone(),
                        reply,
                    )
                },
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        match result {
            CallResult::Success(protocol::TypedRpcResult::Success(value)) => {
                println!(
                    "{} Response from {} on {}:",
                    "✓".green().bold(),
                    actor.green(),
                    node_name.yellow()
                );
                println!();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| format!("{:?}", value))
                );
            }
            CallResult::Success(protocol::TypedRpcResult::ActorNotFound) => {
                return Err(ShellError::RemoteActorNotFound {
                    actor,
                    node: node_name,
                });
            }
            CallResult::Success(protocol::TypedRpcResult::NotSchemaEnabled) => {
                println!(
                    "{} {} is not schema-enabled on {}",
                    "!".yellow().bold(),
                    actor.yellow(),
                    node_name.yellow()
                );
            }
            CallResult::Success(protocol::TypedRpcResult::UnknownVariant(v)) => {
                println!(
                    "{} Unknown RPC variant '{}' for {}",
                    "✗".red().bold(),
                    v.red(),
                    actor.yellow()
                );
            }
            CallResult::Success(protocol::TypedRpcResult::CallFailed(err)) => {
                println!(
                    "{} Call to {} failed: {}",
                    "✗".red().bold(),
                    actor.yellow(),
                    err.red()
                );
            }
            CallResult::Timeout => {
                return Err(ShellError::rpc_timeout());
            }
            CallResult::SenderError => {
                return Err(ShellError::RpcSenderError);
            }
        }

        Ok(())
    }

    /// Send a dynamic message to an actor on a remote node.
    pub(crate) async fn cmd_send_remote(
        &self,
        node_name: String,
        actor: String,
        json_value: serde_json::Value,
    ) -> ShellResult<()> {
        let introspection_actor = self
            .connected_nodes
            .get(&node_name)
            .ok_or_else(|| ShellError::NodeNotConnected(node_name.clone()))?;

        // Send via IntrospectionActor
        let result = introspection_actor
            .call(
                |reply| {
                    ShellProtocolMessage::SendDynamicMessage(
                        actor.clone(),
                        json_value.clone(),
                        reply,
                    )
                },
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        match result {
            CallResult::Success(protocol::DynamicSendResult::Success) => {
                println!(
                    "{} Message sent to {} on {}",
                    "✓".green().bold(),
                    actor.green(),
                    node_name.yellow()
                );
                println!();
                println!("{}", "Message:".bold());
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_value)
                        .unwrap_or_default()
                        .bright_black()
                );
            }
            CallResult::Success(protocol::DynamicSendResult::ActorNotFound) => {
                return Err(ShellError::RemoteActorNotFound {
                    actor,
                    node: node_name,
                });
            }
            CallResult::Success(protocol::DynamicSendResult::NotDynamic) => {
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
            }
            CallResult::Success(protocol::DynamicSendResult::SendFailed(err)) => {
                return Err(ShellError::RemoteOperationFailed(format!(
                    "Failed to send message: {}",
                    err
                )));
            }
            CallResult::Timeout => {
                return Err(ShellError::rpc_timeout());
            }
            CallResult::SenderError => {
                return Err(ShellError::RpcSenderError);
            }
        }

        Ok(())
    }

    /// Call an actor on a remote node with a dynamic message.
    pub(crate) async fn cmd_call_remote(
        &self,
        node_name: String,
        actor: String,
        json_value: serde_json::Value,
    ) -> ShellResult<()> {
        let introspection_actor = self
            .connected_nodes
            .get(&node_name)
            .ok_or_else(|| ShellError::NodeNotConnected(node_name.clone()))?;

        // Call via IntrospectionActor
        let result = introspection_actor
            .call(
                |reply| {
                    ShellProtocolMessage::CallDynamicMessage(
                        actor.clone(),
                        json_value.clone(),
                        reply,
                    )
                },
                Some(DEFAULT_RPC_TIMEOUT),
            )
            .await
            .map_err(ShellError::messaging)?;

        match result {
            CallResult::Success(protocol::DynamicCallResult::Success(response)) => {
                println!(
                    "{} RPC response from {} on {}:",
                    "✓".green().bold(),
                    actor.green(),
                    node_name.yellow()
                );
                println!();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default()
                        .bright_white()
                );
            }
            CallResult::Success(protocol::DynamicCallResult::Error(err)) => {
                println!("{} Actor returned error: {}", "✗".red().bold(), err.red());
            }
            CallResult::Success(protocol::DynamicCallResult::ActorNotFound) => {
                return Err(ShellError::RemoteActorNotFound {
                    actor,
                    node: node_name,
                });
            }
            CallResult::Success(protocol::DynamicCallResult::NotDynamic) => {
                println!(
                    "{}",
                    "⚠ This actor doesn't support dynamic messages".yellow()
                );
                println!();
                println!(
                    "  {} The actor must use DynamicMessage as its message type.",
                    "•".bright_black()
                );
                println!(
                    "  {} Handle DynamicMessage::Call variant to respond to RPC calls.",
                    "•".bright_black()
                );
            }
            CallResult::Success(protocol::DynamicCallResult::CallFailed(err)) => {
                return Err(ShellError::RemoteOperationFailed(format!(
                    "RPC call failed: {}",
                    err
                )));
            }
            CallResult::Timeout => {
                return Err(ShellError::rpc_timeout());
            }
            CallResult::SenderError => {
                return Err(ShellError::RpcSenderError);
            }
        }

        Ok(())
    }

    /// Send a message from a JSON file to an actor.
    pub(crate) async fn cmd_send_file(&self, actor: String, file_path: String) -> ShellResult<()> {
        println!(
            "send-file {} <- {}",
            actor.green(),
            file_path.bright_black()
        );
        println!();

        // Read the file
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => {
                println!("{} Failed to read file '{}': {}", "✗".red(), file_path, e);
                return Ok(());
            }
        };

        println!(
            "{} Read {} bytes from {}",
            "✓".green(),
            content.len(),
            file_path.bright_black()
        );
        println!();

        // Use cmd_send to actually process the message
        self.cmd_send(actor, content).await
    }
}

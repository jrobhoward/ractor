//! Introspection Actor for Remote Shell Queries
//!
//! This module provides the [`IntrospectionActor`] which enables remote shells to query
//! actor information across distributed nodes.
//!
//! ## Usage
//!
//! When connecting to a remote node, the shell looks for `IntrospectionActor` instances
//! in the well-known process group [`INTROSPECTION_GROUP`]. These actors respond to
//! queries about registered actors, process groups, and cluster topology.
//!
//! ```rust,ignore
//! use ractor::Actor;
//! use ractor_shell::introspection::IntrospectionActor;
//!
//! // Spawn an introspection actor on your node
//! let (actor_ref, _) = Actor::spawn(
//!     Some("introspection".to_string()),
//!     IntrospectionActor,
//!     "my_node".to_string(),
//! ).await?;
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::DateTime;

use ractor::rpc::CallResult;
use ractor::{pg, registry, Actor, ActorProcessingErr, ActorRef};

use crate::dynamic::{supports_dynamic_messages, CallResponse, DynamicMessage};
use crate::protocol::{
    ActorInfo, ActorLocation, ClusterTopology, DynamicCallResult, DynamicSendResult, NodeInfo,
    SerializableTraceEvent, ShellProtocolMessage, SubscriptionId, TraceEventBatch, TypedRpcResult,
};
use crate::tracing::{TraceEvent, TraceEventType, TraceFilter, TracingHandle};
use crate::DEFAULT_RPC_TIMEOUT;

/// Well-known process group for shell introspection actors
pub const INTROSPECTION_GROUP: &str = "ractor_shell_introspection";

/// Maximum number of trace events to buffer per subscription before discarding
const MAX_TRACE_BUFFER_SIZE: usize = 1000;

/// Global counter for generating unique subscription IDs
static NEXT_SUBSCRIPTION_ID: AtomicU64 = AtomicU64::new(1);

/// A trace subscription with bounded buffer
struct TraceSubscription {
    /// Pattern for filtering trace events (e.g., "raft_*" or "*")
    pattern: String,
    /// Buffered trace events (bounded queue)
    buffer: VecDeque<TraceEvent>,
    /// Number of events dropped due to buffer overflow since last poll
    dropped_count: usize,
}

impl TraceSubscription {
    fn new(pattern: String) -> Self {
        Self {
            pattern,
            buffer: VecDeque::with_capacity(MAX_TRACE_BUFFER_SIZE),
            dropped_count: 0,
        }
    }

    /// Add an event to the buffer, dropping oldest if full
    fn push_event(&mut self, event: TraceEvent) {
        if self.buffer.len() >= MAX_TRACE_BUFFER_SIZE {
            // Drop oldest event
            self.buffer.pop_front();
            self.dropped_count += 1;
        }
        self.buffer.push_back(event);
    }

    /// Take all buffered events and reset dropped count
    fn take_events(&mut self) -> (Vec<TraceEvent>, usize) {
        let events: Vec<_> = self.buffer.drain(..).collect();
        let dropped = self.dropped_count;
        self.dropped_count = 0;
        (events, dropped)
    }
}

/// Actor that provides introspection capabilities for remote shells.
///
/// This actor responds to [`ShellProtocolMessage`] queries, allowing remote shells
/// to inspect actors, process groups, and cluster topology on this node.
pub struct IntrospectionActor;

/// State for the introspection actor
pub struct IntrospectionState {
    /// Name of this node in the cluster
    pub node_name: String,
    /// Active trace subscriptions
    subscriptions: HashMap<SubscriptionId, TraceSubscription>,
    /// Tracing handle for controlling remote tracing
    tracing_handle: Option<TracingHandle>,
}

/// Arguments for spawning an IntrospectionActor
pub struct IntrospectionArgs {
    /// Name of this node in the cluster
    pub node_name: String,
    /// Optional tracing handle to enable remote tracing subscriptions
    pub tracing_handle: Option<TracingHandle>,
}

impl From<String> for IntrospectionArgs {
    fn from(node_name: String) -> Self {
        Self {
            node_name,
            tracing_handle: None,
        }
    }
}

impl Actor for IntrospectionActor {
    type Msg = ShellProtocolMessage;
    type State = IntrospectionState;
    type Arguments = IntrospectionArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: IntrospectionArgs,
    ) -> Result<Self::State, ActorProcessingErr> {
        // Join the well-known introspection group
        pg::join(INTROSPECTION_GROUP.to_string(), vec![myself.get_cell()]);

        // Set up trace event forwarding if tracing handle provided
        if let Some(ref tracing_handle) = args.tracing_handle {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tracing_handle.set_event_sender(tx);

            // Spawn task to forward trace events to this actor
            let actor_ref = myself.clone();
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    let serializable = SerializableTraceEvent::from_trace_event(&event);
                    let _ =
                        actor_ref.cast(ShellProtocolMessage::TraceEventNotification(serializable));
                }
            });
        }

        Ok(IntrospectionState {
            node_name: args.node_name,
            subscriptions: HashMap::new(),
            tracing_handle: args.tracing_handle,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ShellProtocolMessage::ListRegisteredActors(reply) => {
                let registered = registry::registered();
                let mut actors = Vec::new();

                for name in registered {
                    if let Some(cell) = registry::where_is(name) {
                        actors.push(ActorInfo::from_cell(&cell));
                    }
                }

                let _ = reply.send(actors);
            }

            ShellProtocolMessage::GetProcessGroupMembers(group, reply) => {
                let members = pg::get_members(&group);
                let actors: Vec<ActorInfo> = members.iter().map(ActorInfo::from_cell).collect();

                let _ = reply.send(actors);
            }

            ShellProtocolMessage::GetActorInfo(name, reply) => {
                let info = registry::where_is(name).map(|cell| ActorInfo::from_cell(&cell));

                let _ = reply.send(info);
            }

            ShellProtocolMessage::StopActor(name) => {
                if let Some(cell) = registry::where_is(name) {
                    cell.stop(Some("Stopped by remote shell".to_string()));
                }
            }

            ShellProtocolMessage::Ping(reply) => {
                let _ = reply.send(format!("pong from {}", state.node_name));
            }

            ShellProtocolMessage::GetClusterTopology(reply) => {
                let topology = build_cluster_topology(&state.node_name);
                let _ = reply.send(topology);
            }

            ShellProtocolMessage::SendDynamicMessage(actor_name, json_value, reply) => {
                let result = send_dynamic_message_to_actor(&actor_name, json_value).await;
                let _ = reply.send(result);
            }

            ShellProtocolMessage::CallDynamicMessage(actor_name, json_value, reply) => {
                let result = call_dynamic_message_to_actor(&actor_name, json_value).await;
                let _ = reply.send(result);
            }

            // ==================== Schema Introspection ====================
            ShellProtocolMessage::GetMessageSchema(actor_name, reply) => {
                let schema = crate::schema_registry::get_schema(&actor_name);
                let _ = reply.send(schema);
            }

            ShellProtocolMessage::ListSchemaActors(reply) => {
                let schemas = crate::schema_registry::list_schemas();
                let schema_actors: Vec<crate::protocol::SchemaActorInfo> = schemas
                    .into_iter()
                    .map(|(name, schema)| crate::protocol::SchemaActorInfo { name, schema })
                    .collect();
                let _ = reply.send(schema_actors);
            }

            // ==================== Typed RPC ====================
            ShellProtocolMessage::CallTypedRpc(actor_name, variant_name, args, reply) => {
                let result = call_typed_rpc_on_actor(&actor_name, &variant_name, args).await;
                let _ = reply.send(result);
            }

            // ==================== Remote Tracing ====================
            ShellProtocolMessage::SubscribeToTraces(pattern, reply) => {
                let sub_id = NEXT_SUBSCRIPTION_ID.fetch_add(1, Ordering::SeqCst);

                // Enable tracing for this pattern on the local tracing handle
                if let Some(ref tracing_handle) = state.tracing_handle {
                    tracing_handle.trace(&pattern);
                }

                state
                    .subscriptions
                    .insert(sub_id, TraceSubscription::new(pattern.clone()));

                let _ = reply.send(sub_id);
            }

            ShellProtocolMessage::PollTraces(sub_id, reply) => {
                if let Some(subscription) = state.subscriptions.get_mut(&sub_id) {
                    let (events, dropped_count) = subscription.take_events();

                    let serializable_events: Vec<SerializableTraceEvent> = events
                        .iter()
                        .map(|e| SerializableTraceEvent::from_trace_event(e))
                        .collect();

                    let batch = TraceEventBatch {
                        events: serializable_events,
                        dropped_count,
                    };
                    let _ = reply.send(batch);
                } else {
                    // Subscription not found - send empty batch
                    let _ = reply.send(TraceEventBatch {
                        events: vec![],
                        dropped_count: 0,
                    });
                }
            }

            ShellProtocolMessage::UnsubscribeFromTraces(sub_id, reply) => {
                if let Some(removed_sub) = state.subscriptions.remove(&sub_id) {
                    // Check if any other subscriptions still use this pattern
                    let pattern_still_used = state
                        .subscriptions
                        .values()
                        .any(|s| s.pattern == removed_sub.pattern);

                    // If no other subscriptions use this pattern, disable tracing for it
                    if !pattern_still_used {
                        if let Some(ref tracing_handle) = state.tracing_handle {
                            tracing_handle.untrace(&removed_sub.pattern);
                        }
                    }

                    let _ = reply.send(true);
                } else {
                    let _ = reply.send(false);
                }
            }

            ShellProtocolMessage::TraceEventNotification(serializable_event) => {
                // Convert serializable event back to TraceEvent for internal storage
                // (We store the local format to avoid repeated conversions)
                let event = TraceEvent {
                    timestamp: DateTime::parse_from_rfc3339(&serializable_event.timestamp)
                        .ok()
                        .map(|dt| dt.with_timezone(&chrono::Local))
                        .unwrap_or_else(chrono::Local::now),
                    actor_id: serializable_event.actor_id.clone(),
                    actor_name: serializable_event.actor_name.clone(),
                    event_type: match serializable_event.event_type.as_str() {
                        "ENTER" => TraceEventType::SpanEnter,
                        "EXIT" => TraceEventType::SpanExit,
                        _ => TraceEventType::Event,
                    },
                    level: match serializable_event.level.to_lowercase().as_str() {
                        "trace" => tracing::Level::TRACE,
                        "debug" => tracing::Level::DEBUG,
                        "info" => tracing::Level::INFO,
                        "warn" => tracing::Level::WARN,
                        "error" => tracing::Level::ERROR,
                        _ => tracing::Level::INFO,
                    },
                    target: serializable_event.target.clone(),
                    message: serializable_event.message.clone(),
                    fields: serializable_event.fields.clone(),
                };

                // Distribute event to matching subscriptions
                for subscription in state.subscriptions.values_mut() {
                    if matches_pattern(&subscription.pattern, &event) {
                        subscription.push_event(event.clone());
                    }
                }
            }

            // ==================== Supervision Tree ====================
            ShellProtocolMessage::GetSupervisionTree(actor_name, reply) => {
                use crate::protocol::SupervisionTreeNode;

                let tree_nodes = match actor_name {
                    Some(name) => {
                        // Build tree starting from specific actor
                        if let Some(cell) = registry::where_is(name) {
                            vec![SupervisionTreeNode::from_cell(&cell)]
                        } else {
                            vec![]
                        }
                    }
                    None => {
                        // Find all root actors (those without supervisors) and build trees
                        build_supervision_tree_roots()
                    }
                };

                let _ = reply.send(tree_nodes);
            }

            ShellProtocolMessage::GetActorParent(actor_name, reply) => {
                let parent_info = registry::where_is(actor_name)
                    .and_then(|cell| cell.try_get_supervisor())
                    .map(|supervisor_cell| ActorInfo::from_cell(&supervisor_cell));

                let _ = reply.send(parent_info);
            }
        }

        Ok(())
    }
}

/// Send a dynamic message to an actor (cast - fire and forget)
async fn send_dynamic_message_to_actor(
    actor_name: &str,
    json_value: serde_json::Value,
) -> DynamicSendResult {
    // Find the actor in registry
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return DynamicSendResult::ActorNotFound;
    };

    // Convert to dynamic message actor reference
    let dynamic_ref: ActorRef<DynamicMessage> = ActorRef::from(cell.clone());

    // Check if the actor supports dynamic messages
    if !supports_dynamic_messages(dynamic_ref.clone()).await {
        return DynamicSendResult::NotDynamic;
    }

    // Send the message
    match dynamic_ref.cast(DynamicMessage::Cast(json_value)) {
        Ok(()) => DynamicSendResult::Success,
        Err(e) => DynamicSendResult::SendFailed(format!("{:?}", e)),
    }
}

/// Call an actor with a dynamic message (RPC - wait for response)
async fn call_dynamic_message_to_actor(
    actor_name: &str,
    json_value: serde_json::Value,
) -> DynamicCallResult {
    // Find the actor in registry
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return DynamicCallResult::ActorNotFound;
    };

    // Convert to dynamic message actor reference
    let dynamic_ref: ActorRef<DynamicMessage> = ActorRef::from(cell.clone());

    // Check if the actor supports dynamic messages
    if !supports_dynamic_messages(dynamic_ref.clone()).await {
        return DynamicCallResult::NotDynamic;
    }

    // Make the RPC call
    let call_result = dynamic_ref
        .call(
            |reply| DynamicMessage::Call(json_value, reply),
            Some(DEFAULT_RPC_TIMEOUT),
        )
        .await;

    match call_result {
        Ok(CallResult::Success(response)) => match response {
            CallResponse::Success(value) => DynamicCallResult::Success(value),
            CallResponse::Error(err) => DynamicCallResult::Error(err),
        },
        Ok(CallResult::Timeout) => {
            DynamicCallResult::CallFailed(format!("Timeout after {:?}", DEFAULT_RPC_TIMEOUT))
        }
        Ok(CallResult::SenderError) => {
            DynamicCallResult::CallFailed("Sender error (actor may have stopped)".to_string())
        }
        Err(e) => DynamicCallResult::CallFailed(format!("{:?}", e)),
    }
}

/// Call a typed RPC on a schema-enabled actor
///
/// Note: Typed RPC dispatch requires compile-time knowledge of the message type.
/// Since actors define their own message types in user code (not the library),
/// this function currently only supports schema introspection (listing variants)
/// but not actual RPC calls. Use the `call` shell command with JSON messages
/// for actors that implement `DynamicMessage`.
async fn call_typed_rpc_on_actor(
    actor_name: &str,
    variant_name: &str,
    args: serde_json::Value,
) -> TypedRpcResult {
    // Check if the actor has a registered schema
    if !crate::schema_registry::has_schema(actor_name) {
        return TypedRpcResult::NotSchemaEnabled;
    }

    // Find the actor in registry
    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return TypedRpcResult::ActorNotFound;
    };

    // Try dispatcher-based RPC first
    if let Some(dispatcher) = crate::schema_registry::get_dispatcher(actor_name) {
        match dispatcher(cell, variant_name.to_string(), args).await {
            Ok(result) => return TypedRpcResult::Success(result),
            Err(e) => return TypedRpcResult::CallFailed(e),
        }
    }

    // Fallback: no dispatcher available
    TypedRpcResult::CallFailed(format!(
        "Typed RPC for '{}::{}' not available via introspection. \
         Actor has schema but no dispatcher. Add #[ractor_shell] to enable RPC.",
        actor_name, variant_name
    ))
}

/// Check if a trace event matches a subscription pattern
fn matches_pattern(pattern: &str, event: &TraceEvent) -> bool {
    // Special case: "*" matches everything
    if pattern == "*" {
        return true;
    }

    // Check actor name if present
    if let Some(actor_name) = &event.actor_name {
        if wildcard_match(pattern, actor_name) {
            return true;
        }
    }

    // Check actor ID if present
    if let Some(actor_id) = &event.actor_id {
        if wildcard_match(pattern, actor_id) {
            return true;
        }
    }

    // Check target (module path) - allows patterns like "*raft*" to match "ractor_shell::raft"
    if wildcard_match(pattern, &event.target) {
        return true;
    }

    false
}

/// Simple wildcard matching (supports * and ? wildcards)
fn wildcard_match(pattern: &str, text: &str) -> bool {
    // Use the TraceFilter's matching logic
    let mut filter = TraceFilter::new();
    filter.add_pattern(pattern);
    filter.matches(Some(text))
}

/// Build cluster topology by examining process groups and actor IDs
fn build_cluster_topology(local_node_name: &str) -> ClusterTopology {
    let mut node_ids: HashSet<String> = HashSet::new();
    let mut node_names: HashMap<String, String> = HashMap::new();
    let mut process_groups: HashMap<String, Vec<ActorLocation>> = HashMap::new();

    // Discover nodes and process groups by examining well-known groups
    // In a real implementation, ractor_cluster would provide a proper API for this
    let known_groups = vec!["ping_pong".to_string(), INTROSPECTION_GROUP.to_string()];

    for group_name in known_groups {
        let members = pg::get_members(&group_name);
        let mut locations = Vec::new();

        for cell in members {
            let actor_id = cell.get_id();
            let node_id = extract_node_id(&actor_id.to_string());

            node_ids.insert(node_id.clone());

            // Try to extract node name from registered actors
            if let Some(_name) = cell.get_name() {
                // If it's an introspection actor, extract the node name from state
                // For now, we'll use the node_id as a fallback
                if !node_names.contains_key(&node_id) {
                    node_names.insert(node_id.clone(), format!("node_{}", node_id));
                }
            }

            locations.push(ActorLocation {
                actor_id: actor_id.to_string(),
                actor_name: cell.get_name(),
                node_id: node_id.clone(),
                node_name: node_names
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_else(|| format!("node_{}", node_id)),
            });
        }

        if !locations.is_empty() {
            process_groups.insert(group_name, locations);
        }
    }

    // Build node list
    let registered = registry::registered();
    let local_actor_count = registered.len();

    // Get local node ID from any local actor
    let local_node_id = registered
        .first()
        .and_then(|name| registry::where_is(name.clone()))
        .map(|cell| extract_node_id(&cell.get_id().to_string()))
        .unwrap_or_else(|| "0".to_string());

    node_ids.insert(local_node_id.clone());

    let mut nodes: Vec<NodeInfo> = node_ids
        .iter()
        .map(|node_id| {
            let is_local = node_id == &local_node_id;
            NodeInfo {
                id: node_id.clone(),
                name: if is_local {
                    local_node_name.to_string()
                } else {
                    node_names
                        .get(node_id)
                        .cloned()
                        .unwrap_or_else(|| format!("node_{}", node_id))
                },
                actor_count: if is_local { local_actor_count } else { 0 },
                is_local,
            }
        })
        .collect();

    // Sort nodes by ID for consistent output
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    ClusterTopology {
        nodes,
        process_groups,
    }
}

/// Extract node ID from an actor ID string (e.g., "1.0" -> "1")
fn extract_node_id(actor_id: &str) -> String {
    actor_id.split('.').next().unwrap_or("0").to_string()
}

/// Build supervision trees for all root actors (actors without supervisors)
fn build_supervision_tree_roots() -> Vec<crate::protocol::SupervisionTreeNode> {
    use std::collections::HashSet;

    let mut all_actors = Vec::new();
    let mut seen_ids = HashSet::new();

    // Collect all actors from registry
    for name in registry::registered() {
        if let Some(cell) = registry::where_is(name) {
            let id = cell.get_id();
            if seen_ids.insert(id) {
                all_actors.push(cell);
            }
        }
    }

    // Collect all actors from known process groups
    let known_groups = [
        "ping_pong",
        "ractor_shell_introspection",
        "demo_group",
        "raft_cluster",
    ];
    for group in &known_groups {
        for cell in pg::get_members(&group.to_string()) {
            let id = cell.get_id();
            if seen_ids.insert(id) {
                all_actors.push(cell);
            }
        }
    }

    // Filter to only root actors (those without supervisors)
    let roots: Vec<_> = all_actors
        .iter()
        .filter(|cell| cell.try_get_supervisor().is_none())
        .collect();

    // Build tree nodes for each root
    roots
        .iter()
        .map(|cell| crate::protocol::SupervisionTreeNode::from_cell(cell))
        .collect()
}

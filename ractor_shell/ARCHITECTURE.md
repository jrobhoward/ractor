# Ractor Shell Architecture

This document describes the internal architecture and design decisions of `ractor_shell`.

## Overview

Ractor Shell is an interactive REPL for debugging and observing Ractor actor systems. It provides local and remote introspection capabilities, actor monitoring, and cluster topology visualization.

## Component Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ractor_shell                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐    ┌─────────────────────────────────────────────────────┐│
│  │   main.rs    │    │                    ShellState                       ││
│  │  Entry Point │───▶│  - current_node: Option<String>                     ││
│  │              │    │  - connected_nodes: HashMap<String, ActorRef>       ││
│  └──────────────┘    │  - cluster_topology: Option<ClusterTopology>        ││
│        │             │  - monitor_actor: Option<ActorRef>                  ││
│        ▼             │  - node_server: Option<ActorRef>                    ││
│  ┌──────────────┐    └─────────────────────────────────────────────────────┘│
│  │  rustyline   │                    │                                      │
│  │   Editor     │                    │ execute(cmd)                         │
│  │              │                    ▼                                      │
│  │ - History    │    ┌─────────────────────────────────────────────────────┐│
│  │ - Completion │    │              Command Dispatcher                     ││
│  │ - Hints      │    │                                                     ││
│  └──────────────┘    │   cmd_actors() │ cmd_registry() │ cmd_info()       ││
│        │             │   cmd_connect()│ cmd_cluster()  │ cmd_monitor()    ││
│        │             │   cmd_send()   │ cmd_call()     │ cmd_stop()       ││
│        ▼             └─────────────────────────────────────────────────────┘│
│  ┌──────────────┐                    │                                      │
│  │ ShellCommand │                    │ Local or Remote?                     │
│  │  parse_line  │                    ▼                                      │
│  └──────────────┘    ┌─────────────────────┬───────────────────────────────┐│
│                      │    Local Path       │        Remote Path            ││
│                      │                     │                               ││
│                      │  ractor::registry   │    IntrospectionActor         ││
│                      │  ractor::pg         │    (via RPC)                  ││
│                      │                     │                               ││
│                      └─────────────────────┴───────────────────────────────┘│
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      │ (remote connection)
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Remote Node                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────┐      ┌────────────────────────────────────────┐ │
│  │   ractor_cluster       │      │        IntrospectionActor              │ │
│  │   NodeSession          │◀────▶│                                        │ │
│  │                        │      │  - Joins INTROSPECTION_GROUP           │ │
│  │  - TCP Connection      │      │  - Responds to ShellProtocolMessage    │ │
│  │  - Message Routing     │      │  - Queries local registry/pg           │ │
│  │  - Authentication      │      │                                        │ │
│  └────────────────────────┘      └────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Key Components

### ShellState

The central coordinator that maintains all shell state:

```rust
pub struct ShellState {
    pub should_exit: bool,
    pub current_node: Option<String>,
    pub local_node_name: String,
    pub node_server: Option<ActorRef<NodeServerMessage>>,
    pub connected_nodes: HashMap<String, ActorRef<ShellProtocolMessage>>,
    pub cluster_topology: Option<ClusterTopology>,
    pub monitor_actor: Option<ActorRef<MonitorMessage>>,
}
```

**Responsibilities:**
- Tracks current node context (local vs remote)
- Manages connections to remote nodes
- Caches cluster topology for performance
- Owns the MonitorActor reference
- Dispatches commands to appropriate handlers

### ShellCommand

Enum representing all possible shell commands:

```rust
pub enum ShellCommand {
    Actors,
    Registry,
    ProcessGroup { subcommand: PgSubcommand },
    Info { actor: String },
    Send { actor: String, message: String },
    Call { actor: String, message: String },
    Stop { actor: String },
    Connect { address: String },
    Disconnect { node: String },
    Nodes,
    Use { node: String },
    Cluster { subcommand: Option<ClusterSubcommand> },
    // ... and more
}
```

**Design Decision:** Using an enum for commands provides:
- Type-safe command handling
- Exhaustive match checking
- Clear documentation of available commands

### IntrospectionActor

Actor that enables remote shells to query actor information:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        IntrospectionActor Flow                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Shell                  Network                    Remote Node               │
│    │                       │                            │                    │
│    │  cmd: "actors"        │                            │                    │
│    │───────────────────────│                            │                    │
│    │                       │                            │                    │
│    │  RPC: ListRegisteredActors                         │                    │
│    │───────────────────────│──────────────────────────▶│                    │
│    │                       │                            │                    │
│    │                       │   IntrospectionActor       │                    │
│    │                       │   receives message         │                    │
│    │                       │            │               │                    │
│    │                       │            ▼               │                    │
│    │                       │   ractor::registry::       │                    │
│    │                       │   registered()             │                    │
│    │                       │            │               │                    │
│    │                       │            ▼               │                    │
│    │                       │   Build Vec<ActorInfo>     │                    │
│    │                       │            │               │                    │
│    │  Vec<ActorInfo>       │◀───────────│               │                    │
│    │◀──────────────────────│                            │                    │
│    │                       │                            │                    │
│    ▼                       │                            │                    │
│  Display table                                                               │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Process Group Discovery:**
The IntrospectionActor joins the well-known process group `ractor_shell_introspection`. This allows shells to discover introspection actors on any connected node:

```rust
async fn pre_start(&self, myself: ActorRef<Self::Msg>, node_name: String) {
    ractor::pg::join(INTROSPECTION_GROUP.to_string(), vec![myself.get_cell()]);
    Ok(IntrospectionState { node_name })
}
```

### MonitorActor

Actor that tracks lifecycle events for monitored actors:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          MonitorActor Flow                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Shell                    MonitorActor              Ractor Runtime           │
│    │                           │                          │                  │
│    │  cmd: "monitor foo"       │                          │                  │
│    │─────────────────────────▶│                          │                  │
│    │                           │                          │                  │
│    │                           │  Lookup "foo" in         │                  │
│    │                           │  registry                │                  │
│    │                           │─────────────────────────▶│                  │
│    │                           │                          │                  │
│    │                           │  ActorCell               │                  │
│    │                           │◀─────────────────────────│                  │
│    │                           │                          │                  │
│    │                           │  Store in monitored map  │                  │
│    │                           │  {name -> actor_id}      │                  │
│    │                           │                          │                  │
│    │  "✓ Monitoring foo"       │                          │                  │
│    │◀─────────────────────────│                          │                  │
│    │                           │                          │                  │
│    │         ... later ...     │                          │                  │
│    │                           │                          │                  │
│    │                           │  SupervisionEvent::      │                  │
│    │                           │  ActorTerminated(foo)    │                  │
│    │                           │◀─────────────────────────│                  │
│    │                           │                          │                  │
│    │  Event formatted          │  Convert to MonitorEvent │                  │
│    │  and displayed            │  if actor is monitored   │                  │
│    │◀─────────────────────────│                          │                  │
│    │                           │                          │                  │
│    ▼                           │                          │                  │
│  [14:24:12] ▼ STOPPED foo                                                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Event Types:**
- `ActorStarted` - Actor began execution
- `ActorStopped` - Actor stopped normally
- `ActorPanicked` - Actor panicked during message handling
- `ActorKilled` - Actor received kill signal

## Command Flow

### Local Command Execution

```
User Input → parse_line() → ShellCommand → execute() → cmd_*() → Output
                                              │
                                              ▼
                                    ractor::registry::*
                                    ractor::pg::*
```

### Remote Command Execution

```
User Input → parse_line() → ShellCommand → execute() → cmd_*()
                                              │
                                              ▼
                                    current_node == Some(node)
                                              │
                                              ▼
                                    connected_nodes.get(node)
                                              │
                                              ▼
                                    RPC call to IntrospectionActor
                                              │
                                              ▼
                                    Output
```

## State Management

### Node Context

The shell maintains a "current node" context that determines where commands execute:

```rust
// Local mode (default)
state.current_node = None;
// Prompt: ractor@local >

// Remote mode
state.current_node = Some("127.0.0.1:9002".to_string());
// Prompt: ractor@127.0.0.1:9002 >
```

Commands check this context to determine execution path:

```rust
async fn cmd_actors(&mut self) -> ShellResult<()> {
    if let Some(ref node) = self.current_node {
        // Remote: use RPC to IntrospectionActor
        self.cmd_actors_remote(node).await
    } else {
        // Local: query ractor directly
        self.cmd_actors_local().await
    }
}
```

### Connection State

Remote connections are tracked in `connected_nodes`:

```rust
// On successful connect
self.connected_nodes.insert(
    address.clone(),
    introspection_actor_ref,
);

// On disconnect
self.connected_nodes.remove(&node);
```

### Cluster Topology Cache

Topology is cached to avoid repeated expensive queries:

```rust
// Refreshed on demand or connection
self.cluster_topology = Some(topology);

// Used by cluster commands
if let Some(ref topology) = self.cluster_topology {
    // Display cached topology
}
```

## Remote Connection Handling

### Connection Sequence

```
1. User: connect 127.0.0.1:9002

2. Shell checks if NodeServer exists
   └─▶ If not: spawn local NodeServer on port 9100

3. Connect to remote NodeServer
   └─▶ ractor_cluster establishes TCP connection

4. Discover IntrospectionActor
   └─▶ Query INTROSPECTION_GROUP process group
   └─▶ Find actor on target node

5. Send Ping to verify connection
   └─▶ "pong from node_name"

6. Discover cluster topology (optional)
   └─▶ GetClusterTopology RPC

7. Store connection in connected_nodes map

8. User can now: use 127.0.0.1:9002
```

### Protocol Messages

The shell protocol is defined in `protocol.rs`:

```rust
pub enum ShellProtocolMessage {
    /// List all registered actors on this node
    ListRegisteredActors(RpcReplyPort<Vec<ActorInfo>>),

    /// Get members of a process group
    GetProcessGroupMembers(String, RpcReplyPort<Vec<ActorInfo>>),

    /// Get detailed info about a specific actor
    GetActorInfo(String, RpcReplyPort<Option<ActorInfo>>),

    /// Stop an actor by name
    StopActor(String),

    /// Ping to verify connection
    Ping(RpcReplyPort<String>),

    /// Get full cluster topology
    GetClusterTopology(RpcReplyPort<ClusterTopology>),
}
```

## Design Decisions

### 1. Named Actors Only

**Decision:** Only show actors registered in the registry (named actors).

**Rationale:** Ractor 0.15 doesn't expose a complete actor enumeration API. The registry provides a reliable source of named actors.

**Trade-off:** Unnamed actors are not visible. Future ractor versions may add introspection APIs.

### 2. Process Groups Over Supervision Trees

**Decision:** Use process groups for actor organization instead of supervision trees.

**Rationale:** Ractor's supervision trees are internal. Process groups are explicitly managed and visible to applications.

**Trade-off:** True supervision hierarchy not visible. Would require ractor core changes.

### 3. Dynamic Message Interface

**Decision:** Actors must opt-in to receive shell messages via `DynamicMessage` type.

**Rationale:** Type safety is fundamental to Rust and ractor. Arbitrary JSON cannot be safely converted to typed messages without actor cooperation.

**Trade-off:** Not all actors can receive shell messages. But this is the correct design for a type-safe system.

### 4. RPC-Based Remote Introspection

**Decision:** Use ractor's RPC mechanism for remote queries rather than custom protocols.

**Rationale:** Leverages existing, tested infrastructure. Provides timeouts and error handling.

**Trade-off:** Requires IntrospectionActor on each node.

### 5. Single Process Group for Discovery

**Decision:** Use a well-known process group (`ractor_shell_introspection`) for actor discovery.

**Rationale:** Process groups are automatically replicated across cluster nodes, providing easy discovery.

**Trade-off:** Naming collision possible (mitigated by unique name).

## Error Handling

All errors use strongly-typed `ShellError`:

```rust
#[derive(Debug, Error)]
pub enum ShellError {
    #[error("Actor '{0}' not found in registry. Use 'registry' to list actors.")]
    ActorNotFound(String),

    #[error("RPC call timed out after {0:?}")]
    RpcTimeout(Duration),

    #[error("Not connected to any node. Use 'connect <host:port>' first.")]
    NotConnected,

    // ... more variants
}
```

**Design Principles:**
- Error messages include actionable suggestions
- Errors are recoverable where possible
- User-facing errors are clear and helpful

## Testing Strategy

See [TESTING.md](TESTING.md) for complete testing conventions.

**Test Categories:**
- Unit tests: Command parsing, error cases
- Integration tests: Actor interactions
- Property tests: Fuzzing with proptest
- Benchmarks: Performance validation

## Future Considerations

### Introspection APIs (Phase 2)

Future ractor versions may add:
- `get_all_actors()` - Complete actor enumeration
- `get_supervision_tree()` - Supervision hierarchy
- Global event subscription

These would enable:
- Full actor visibility
- True supervision tree display
- Real-time event streams

### Performance Optimizations

Potential improvements:
- Connection pooling for remote nodes
- Streaming responses for large datasets
- Background topology refresh

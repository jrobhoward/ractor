# Dynamic Message Interface

This guide explains how to create actors that can receive arbitrary JSON messages from the shell.

## The Problem

Rust's type system requires actors to have strongly-typed message enums. For example:

```rust
enum MyMessage {
    Ping,
    DoWork(String),
}
```

This means the shell cannot send arbitrary JSON to actors without knowing their message type at compile time.

## The Solution: DynamicMessage

Actors that want to receive JSON from the shell can use `DynamicMessage` as their message type.

```rust
use ractor_shell::dynamic::DynamicMessage;

impl Actor for MyActor {
    type Msg = DynamicMessage;  // Use this instead of a custom enum
    // ...
}
```

## DynamicMessage Variants

The `DynamicMessage` enum has three variants:

### 1. Cast - Fire-and-forget Messages

Sent when the user runs: `send <actor> <json>`

```rust
DynamicMessage::Cast(json_value)
```

Example handling:

```rust
async fn handle(&self, _myself: ActorRef<Self::Msg>, message: Self::Msg, state: &mut Self::State)
    -> Result<(), ActorProcessingErr> {
    match message {
        DynamicMessage::Cast(json) => {
            if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                match cmd {
                    "increment" => {
                        state.counter += 1;
                        Ok(())
                    }
                    _ => Ok(())
                }
            } else {
                Ok(())
            }
        }
        // ... other variants
    }
}
```

### 2. Call - RPC Messages

Sent when the user runs: `call <actor> <json>`

```rust
DynamicMessage::Call(json_value, reply_port)
```

Example handling:

```rust
DynamicMessage::Call(json, reply) => {
    if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
        match cmd {
            "get_status" => {
                let response = json!({
                    "status": state.status,
                    "counter": state.counter
                });
                let _ = reply.send(CallResponse::Success(response));
            }
            _ => {
                let _ = reply.send(CallResponse::Error(
                    "Unknown command".to_string()
                ));
            }
        }
    }
    Ok(())
}
```

### 3. Ping - Detection Message

Used by the shell to detect if an actor supports dynamic messages.

```rust
DynamicMessage::Ping(reply) => {
    let _ = reply.send(true);
    Ok(())
}
```

Always reply with `true` to indicate support.

## Complete Example

```rust
use ractor::{Actor, ActorProcessingErr, ActorRef};
use ractor_shell::dynamic::{DynamicMessage, CallResponse};
use serde_json::json;

struct CounterActor;

struct CounterState {
    count: i32,
}

impl Actor for CounterActor {
    type Msg = DynamicMessage;
    type State = CounterState;
    type Arguments = ();

    async fn pre_start(&self, _myself: ActorRef<Self::Msg>, _: ())
        -> Result<Self::State, ActorProcessingErr> {
        Ok(CounterState { count: 0 })
    }

    async fn handle(&self, _myself: ActorRef<Self::Msg>, message: Self::Msg, state: &mut Self::State)
        -> Result<(), ActorProcessingErr> {
        match message {
            // Handle cast messages
            DynamicMessage::Cast(json) => {
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "increment" => state.count += 1,
                        "decrement" => state.count -= 1,
                        "reset" => state.count = 0,
                        _ => {}
                    }
                }
                Ok(())
            }

            // Handle RPC messages
            DynamicMessage::Call(json, reply) => {
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    match cmd {
                        "get" => {
                            let _ = reply.send(CallResponse::Success(
                                json!({"count": state.count})
                            ));
                        }
                        "add" => {
                            if let Some(amount) = json.get("amount").and_then(|v| v.as_i64()) {
                                state.count += amount as i32;
                                let _ = reply.send(CallResponse::Success(
                                    json!({"count": state.count})
                                ));
                            } else {
                                let _ = reply.send(CallResponse::Error(
                                    "Missing 'amount' field".to_string()
                                ));
                            }
                        }
                        _ => {
                            let _ = reply.send(CallResponse::Error(
                                format!("Unknown command: {}", cmd)
                            ));
                        }
                    }
                } else {
                    let _ = reply.send(CallResponse::Error(
                        "No 'command' field".to_string()
                    ));
                }
                Ok(())
            }

            // Handle ping (detection)
            DynamicMessage::Ping(reply) => {
                let _ = reply.send(true);
                Ok(())
            }
        }
    }
}
```

## Using from the Shell

Once you have a dynamic actor, you can interact with it from the shell:

```
# Send cast messages (fire-and-forget)
ractor@local > send counter {"command": "increment"}
✓ Actor supports dynamic messages
✓ Message sent to counter

# Make RPC calls (request-reply)
ractor@local > call counter {"command": "get"}
✓ Actor supports dynamic messages
• Making RPC call...

✓ RPC call successful

Response:
{
  "count": 1
}
```

## Message Patterns

### Simple Command Pattern

```json
{"command": "do_something"}
```

```rust
if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
    match cmd {
        "do_something" => { /* ... */ }
        _ => {}
    }
}
```

### Command with Data Pattern

```json
{"command": "set_value", "value": 42}
```

```rust
match json.get("command").and_then(|v| v.as_str()) {
    Some("set_value") => {
        if let Some(value) = json.get("value").and_then(|v| v.as_i64()) {
            state.value = value as i32;
        }
    }
    _ => {}
}
```

### Nested Data Pattern

```json
{
  "command": "update_config",
  "config": {
    "timeout": 30,
    "retries": 3,
    "options": ["verbose", "debug"]
  }
}
```

```rust
match json.get("command").and_then(|v| v.as_str()) {
    Some("update_config") => {
        if let Some(config) = json.get("config") {
            if let Some(timeout) = config.get("timeout").and_then(|v| v.as_i64()) {
                state.timeout = timeout as u32;
            }
            // ... handle other fields
        }
    }
    _ => {}
}
```

## Error Handling

### Cast Messages

Cast messages are fire-and-forget, so errors should be logged but not crash the actor:

```rust
DynamicMessage::Cast(json) => {
    if let Err(e) = self.process_cast(&json, state) {
        eprintln!("Error processing cast: {}", e);
    }
    Ok(())  // Don't crash the actor
}
```

### Call Messages

Call messages should send error responses:

```rust
DynamicMessage::Call(json, reply) => {
    match self.process_call(&json, state) {
        Ok(result) => {
            let _ = reply.send(CallResponse::Success(result));
        }
        Err(e) => {
            let _ = reply.send(CallResponse::Error(e.to_string()));
        }
    }
    Ok(())
}
```

## Trade-offs

### Advantages

- ✅ Actors can be controlled dynamically from the shell
- ✅ No recompilation needed to send new message types
- ✅ Great for debugging and experimentation
- ✅ Works well for admin/control interfaces

### Disadvantages

- ⚠️ Less type safety - errors caught at runtime instead of compile time
- ⚠️ More verbose message handling code
- ⚠️ Need to manually parse JSON fields
- ⚠️ Can't use Rust's enum exhaustiveness checking

## When to Use

**Use DynamicMessage when:**
- Building admin or debugging tools
- Prototyping and experimentation
- Need runtime flexibility
- Actors need to be controllable from external systems

**Use strongly-typed messages when:**
- Production code with known message types
- Want compile-time type safety
- Performance is critical
- Messages are well-defined and stable

## Hybrid Approach

You can have both dynamic and typed actors in the same system:

```rust
// Critical business logic - strongly typed
impl Actor for OrderProcessor {
    type Msg = OrderMessage;
    // ...
}

// Admin interface - dynamic
impl Actor for AdminActor {
    type Msg = DynamicMessage;
    // ...
}
```

This gives you type safety where it matters and flexibility where you need it.

## Alternative: Schema-Enabled Typed Messages

For actors that need efficient binary serialization over the cluster network (like Raft), `DynamicMessage` adds overhead. Instead, you can use typed messages with schema introspection:

```rust
use ractor_cluster_derive::RactorClusterMessage;

#[derive(RactorClusterMessage, Debug)]
#[ractor_shell]  // Generates SchemaProvider for shell introspection
pub enum MyMessage {
    // Internal messages (binary over cluster)
    DoWork(u64, String),

    // Shell-queryable RPC variants
    #[rpc] GetStatus(RpcReplyPort<MyStatus>),
    #[rpc] GetValue(RpcReplyPort<i32>),
}

impl Actor for MyActor {
    type Msg = MyMessage;

    async fn pre_start(&self, myself: ActorRef<Self::Msg>, _args: ()) -> Result<Self::State, ActorProcessingErr> {
        // Register schema for shell introspection
        if let Some(name) = myself.get_name() {
            ractor_shell::schema_registry::register::<MyMessage>(&name);
        }
        Ok(MyState::default())
    }
}
```

From the shell, use typed syntax:
```
ractor@local > call my_actor GetStatus {}
ractor@local > call my_actor GetValue {}
```

**When to use typed messages:**
- Cluster actors that need binary serialization
- Performance-critical message paths
- Actors with well-defined RPC interfaces

**When to use DynamicMessage:**
- Prototyping and experimentation
- Actors that need maximum flexibility
- Admin interfaces with evolving commands

## See Also

- [`examples/cluster_demo/raft.rs`](../examples/cluster_demo/raft.rs) - Schema-enabled typed message example (RaftMessage)
- [`src/schema_registry.rs`](../src/schema_registry.rs) - Schema registration implementation

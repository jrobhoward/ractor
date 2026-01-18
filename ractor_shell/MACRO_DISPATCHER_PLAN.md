# Macro-Generated RPC Dispatcher Implementation Plan

**Goal**: Enable remote typed RPC calls to actors in example code by having the `#[ractor_shell]` macro generate dispatcher functions that bridge JSON → typed messages.

**Estimated Time**: 2-3 hours
**Difficulty**: Medium
**Benefits**:
- ✅ Type-safe typed RPC for all `#[ractor_shell]` actors
- ✅ Clean architecture (library doesn't import example code)
- ✅ Works for both local and remote actors
- ✅ Automatic - just add `#[ractor_shell]` attribute

---

## Problem Statement

Currently, `call raft_node GetStatus {}` fails with:
```
✗ Typed RPC for 'raft_node::GetStatus' not available via introspection
```

This is because:
1. RaftNode is in `examples/` (not library code)
2. `introspection.rs` can't `use` example types
3. The old hard-coded dispatch was removed in commit `9bb4a66`

**Solution**: Have the `#[ractor_shell]` macro generate type-safe dispatcher functions that can be registered with the schema registry and called by the introspection layer.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│ User Code (examples/cluster_demo/raft.rs)                  │
│                                                             │
│  #[derive(RactorClusterMessage)]                           │
│  #[ractor_shell]  ← Macro generates dispatcher             │
│  pub enum RaftMessage { ... }                              │
│                                                             │
│  impl Actor for RaftNode {                                 │
│    async fn pre_start(...) {                               │
│      // Register generated dispatcher                      │
│      schema_registry::register_with_dispatcher::<          │
│        RaftMessage>(&name);                                │
│    }                                                        │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ Registers dispatcher function
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ ractor_shell::schema_registry                              │
│                                                             │
│  - Stores: actor_name → (Schema, Dispatcher)               │
│  - Dispatcher = async fn(ActorCell, variant, args) → JSON │
└─────────────────────────────────────────────────────────────┘
                          ↑
                          │ Looks up and calls dispatcher
                          │
┌─────────────────────────────────────────────────────────────┐
│ ractor_shell::introspection                                │
│                                                             │
│  async fn call_typed_rpc_on_actor(...) {                   │
│    if let Some(dispatcher) = get_dispatcher(actor_name) {  │
│      return dispatcher(cell, variant, args).await;         │
│    }                                                        │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘
```

---

## Implementation Steps

### Step 1: Enhance Macro to Generate Dispatcher (2 hours)

**File**: `ractor_cluster_derive/src/lib.rs`

**Current behavior**:
- `#[ractor_shell]` generates `SchemaProvider` impl
- Provides schema metadata (variants, fields, types)

**New behavior**:
- Also generate `__ractor_shell_dispatcher` module
- Contains async dispatcher function for RPC calls

**Generated code structure**:

```rust
// For this input:
#[derive(RactorClusterMessage)]
#[ractor_shell]
pub enum RaftMessage {
    #[rpc]
    GetStatus(RpcReplyPort<RaftStatus>),
    #[rpc]
    IsLeader(RpcReplyPort<bool>),
    #[rpc]
    GetLeader(RpcReplyPort<Option<String>>),
    #[rpc]
    GetPeers(RpcReplyPort<Vec<String>>),
    // Non-RPC messages ignored for dispatch
    RequestVote(u64, String),
}

// Macro should generate:
#[doc(hidden)]
mod __ractor_shell_raft_message_dispatcher {
    use super::*;
    use ractor::ActorCell;
    use serde_json::Value as JsonValue;

    pub async fn dispatch_rpc(
        cell: ActorCell,
        variant: &str,
        _args: JsonValue,  // For future: parse args from JSON
    ) -> Result<JsonValue, String> {
        // Convert ActorCell to typed ActorRef
        let actor_ref: ractor::ActorRef<RaftMessage> = ractor::ActorRef::from(cell);

        match variant {
            "GetStatus" => {
                let result = ractor::call!(actor_ref, RaftMessage::GetStatus)
                    .map_err(|e| format!("RPC failed: {:?}", e))?;
                serde_json::to_value(&result)
                    .map_err(|e| format!("Serialization failed: {}", e))
            }
            "IsLeader" => {
                let result = ractor::call!(actor_ref, RaftMessage::IsLeader)
                    .map_err(|e| format!("RPC failed: {:?}", e))?;
                Ok(serde_json::json!(result))
            }
            "GetLeader" => {
                let result = ractor::call!(actor_ref, RaftMessage::GetLeader)
                    .map_err(|e| format!("RPC failed: {:?}", e))?;
                serde_json::to_value(&result)
                    .map_err(|e| format!("Serialization failed: {}", e))
            }
            "GetPeers" => {
                let result = ractor::call!(actor_ref, RaftMessage::GetPeers)
                    .map_err(|e| format!("RPC failed: {:?}", e))?;
                serde_json::to_value(&result)
                    .map_err(|e| format!("Serialization failed: {}", e))
            }
            _ => Err(format!("Unknown RPC variant: {}", variant))
        }
    }
}

// Also modify SchemaProvider impl to expose dispatcher:
impl ractor_shell::SchemaProvider for RaftMessage {
    fn schema() -> ractor_shell::ActorSchema {
        // ... existing schema code ...
    }

    fn dispatcher() -> Option<ractor_shell::RpcDispatcher> {
        Some(Box::new(|cell, variant, args| {
            Box::pin(__ractor_shell_raft_message_dispatcher::dispatch_rpc(cell, variant, args))
        }))
    }
}
```

**Macro implementation guidance**:

1. **Parse `#[rpc]` attributes** - Already done for schema generation
2. **Generate dispatcher module** - One `match` arm per `#[rpc]` variant
3. **Handle reply types** - Extract `T` from `RpcReplyPort<T>` to determine serialization
4. **Skip non-RPC variants** - Only generate cases for `#[rpc]` marked variants
5. **Add to SchemaProvider** - New optional `dispatcher()` method

**Code location**: Look for `generate_schema_provider_impl()` function, add similar `generate_dispatcher_impl()`.

**Testing**: Expand macro with `cargo expand -p ractor_shell --example cluster_demo` and verify generated code.

---

### Step 2: Add Dispatcher Registry (30 minutes)

**File**: `ractor_shell/src/schema_registry.rs`

**Current structure**:
```rust
static SCHEMAS: Lazy<RwLock<HashMap<String, ActorSchema>>> = ...;

pub fn register<T: SchemaProvider>(actor_name: &str) {
    SCHEMAS.write().unwrap().insert(actor_name.to_string(), T::schema());
}
```

**New structure**:

```rust
use ractor::ActorCell;

/// Type-erased async RPC dispatcher
pub type RpcDispatcher = Box<
    dyn Fn(ActorCell, String, serde_json::Value)
        -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>
    + Send
    + Sync
>;

struct SchemaEntry {
    schema: ActorSchema,
    dispatcher: Option<RpcDispatcher>,
}

static REGISTRY: Lazy<RwLock<HashMap<String, SchemaEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Register schema with optional dispatcher
pub fn register<T: SchemaProvider>(actor_name: &str) {
    let mut registry = REGISTRY.write().unwrap();
    registry.insert(
        actor_name.to_string(),
        SchemaEntry {
            schema: T::schema(),
            dispatcher: T::dispatcher(),
        },
    );
}

/// Get dispatcher for an actor
pub fn get_dispatcher(actor_name: &str) -> Option<RpcDispatcher> {
    let registry = REGISTRY.read().unwrap();
    registry.get(actor_name).and_then(|entry| entry.dispatcher.clone())
}

/// Check if actor has a dispatcher
pub fn has_dispatcher(actor_name: &str) -> bool {
    let registry = REGISTRY.read().unwrap();
    registry.get(actor_name).map_or(false, |e| e.dispatcher.is_some())
}

/// Get schema (keep existing API)
pub fn get_schema(actor_name: &str) -> Option<ActorSchema> {
    let registry = REGISTRY.read().unwrap();
    registry.get(actor_name).map(|e| e.schema.clone())
}
```

**Also update**:
- `ractor_shell/src/lib.rs` - Export `RpcDispatcher` type
- `ractor_shell/src/dynamic.rs` - Add `pub use` for schema types

---

### Step 3: Update SchemaProvider Trait (15 minutes)

**File**: `ractor_shell/src/lib.rs` (or wherever `SchemaProvider` is defined)

**Current trait**:
```rust
pub trait SchemaProvider {
    fn schema() -> ActorSchema;
}
```

**New trait**:
```rust
pub trait SchemaProvider {
    fn schema() -> ActorSchema;

    /// Optional RPC dispatcher for this message type
    fn dispatcher() -> Option<RpcDispatcher> {
        None  // Default: no dispatcher
    }
}
```

**Update existing impls**: Any existing `SchemaProvider` implementations need to add the default method (or explicitly return `None`).

---

### Step 4: Update Introspection Layer (30 minutes)

**File**: `ractor_shell/src/introspection.rs`

**Find**: `async fn call_typed_rpc_on_actor(...)`

**Current code** (returns error stub):
```rust
async fn call_typed_rpc_on_actor(
    actor_name: &str,
    variant_name: &str,
    args: serde_json::Value,
) -> TypedRpcResult {
    if !crate::schema_registry::has_schema(actor_name) {
        return TypedRpcResult::NotSchemaEnabled;
    }

    let Some(cell) = registry::where_is(actor_name.to_string()) else {
        return TypedRpcResult::ActorNotFound;
    };

    TypedRpcResult::CallFailed(format!(
        "Typed RPC for '{}::{}' not available via introspection. \
         Use DynamicMessage or application-specific handlers.",
        actor_name, variant_name
    ))
}
```

**New code** (call dispatcher if available):
```rust
async fn call_typed_rpc_on_actor(
    actor_name: &str,
    variant_name: &str,
    args: serde_json::Value,
) -> TypedRpcResult {
    if !crate::schema_registry::has_schema(actor_name) {
        return TypedRpcResult::NotSchemaEnabled;
    }

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
```

**Note**: The dispatcher function already handles ActorRef conversion, RPC call, and result serialization.

---

### Step 5: Update RaftNode Registration (5 minutes)

**File**: `ractor_shell/examples/cluster_demo/raft.rs`

**Current code** in `pre_start`:
```rust
// Register schema for shell introspection
if let Some(name) = myself.get_name() {
    ractor_shell::schema_registry::register::<RaftMessage>(&name);
}
```

**No changes needed!** The macro-generated `dispatcher()` method is automatically picked up by `register::<T>()`.

**Verify**: After macro changes, this should "just work" because:
1. `RaftMessage` gets `dispatcher()` from macro
2. `register::<RaftMessage>()` calls `T::dispatcher()`
3. Dispatcher is stored in registry

---

### Step 6: Handle Edge Cases (30 minutes)

#### 6.1 Timeout Handling

Dispatchers should respect RPC timeouts. Update generated code:

```rust
// In macro-generated dispatcher:
pub async fn dispatch_rpc(...) -> Result<JsonValue, String> {
    let actor_ref: ractor::ActorRef<RaftMessage> = ractor::ActorRef::from(cell);

    // Default timeout for shell RPC calls
    let timeout = Some(std::time::Duration::from_secs(5));

    match variant {
        "GetStatus" => {
            match actor_ref.call(
                RaftMessage::GetStatus,
                timeout
            ).await {
                Ok(ractor::rpc::CallResult::Success(result)) => {
                    serde_json::to_value(&result)
                        .map_err(|e| format!("Serialization failed: {}", e))
                }
                Ok(ractor::rpc::CallResult::Timeout) => {
                    Err("RPC timeout".to_string())
                }
                Ok(ractor::rpc::CallResult::SenderError) => {
                    Err("Actor stopped or unreachable".to_string())
                }
                Err(e) => Err(format!("RPC failed: {:?}", e))
            }
        }
        // ... other variants ...
    }
}
```

#### 6.2 Non-Serializable Reply Types

If a reply type doesn't implement `Serialize`, compilation will fail. This is **good** - it's a compile-time error that forces the user to make their types serializable.

**Recommendation**: Document that `#[ractor_shell]` requires all RPC reply types to implement `serde::Serialize`.

#### 6.3 Remote Actor Support

The dispatcher receives an `ActorCell`, which works for both local and remote actors. No special handling needed - it already works!

---

## Testing Strategy

### Unit Tests

**File**: `ractor_cluster_derive/src/tests.rs` (or new file)

```rust
#[test]
fn test_dispatcher_generation() {
    let input = quote! {
        #[derive(RactorClusterMessage)]
        #[ractor_shell]
        pub enum TestMessage {
            #[rpc]
            GetValue(RpcReplyPort<i32>),
        }
    };

    let output = ractor_cluster_message_derive_macro(input.into());

    // Verify dispatcher module is generated
    assert!(output.to_string().contains("__ractor_shell_test_message_dispatcher"));
    assert!(output.to_string().contains("dispatch_rpc"));
}
```

### Integration Tests

**File**: `ractor_shell/tests/typed_rpc_dispatcher.rs` (new file)

```rust
use ractor::rpc::CallResult;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use ractor_cluster::RactorClusterMessage;
use ractor_shell::schema_registry;
use serde::{Deserialize, Serialize};

#[derive(RactorClusterMessage, Debug)]
#[ractor_shell]
enum TestMessage {
    #[rpc]
    GetValue(RpcReplyPort<i32>),
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
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_typed_rpc_via_dispatcher() {
    let (actor_ref, _handle) = Actor::spawn(
        Some("test_actor".to_string()),
        TestActor,
        (),
    )
    .await
    .unwrap();

    // Verify dispatcher was registered
    assert!(schema_registry::has_dispatcher("test_actor"));

    // Get dispatcher and call it
    let dispatcher = schema_registry::get_dispatcher("test_actor").unwrap();
    let result = dispatcher(
        actor_ref.get_cell(),
        "GetValue".to_string(),
        serde_json::json!({}),
    )
    .await
    .unwrap();

    assert_eq!(result, serde_json::json!(42));
}
```

### Manual Testing

**Test with cluster_demo**:

```bash
# Terminal 1: Start cluster
./ractor_shell/scripts/test_cluster.sh

# Terminal 2: Shell
cargo run --example cluster_demo -p ractor_shell -- shell
```

```
ractor@local > connect 127.0.0.1:9001
ractor@127.0.0.1:9001 > call raft_node GetStatus {}
# Should work! Returns JSON with node status

ractor@127.0.0.1:9001 > call raft_node IsLeader {}
# Should return: {"is_leader": false}  (or true)

ractor@127.0.0.1:9001 > call raft_node GetPeers {}
# Should return: {"peers": ["node_b", "node_c"]}
```

---

## Files to Modify

### Core Changes

1. ✏️ **`ractor_cluster_derive/src/lib.rs`**
   - Enhance `ractor_cluster_message_derive_macro()`
   - Add dispatcher module generation
   - Add `dispatcher()` method to `SchemaProvider` impl
   - ~100-150 lines of code changes

2. ✏️ **`ractor_shell/src/schema_registry.rs`**
   - Change `HashMap<String, ActorSchema>` to `HashMap<String, SchemaEntry>`
   - Add `RpcDispatcher` type definition
   - Add `get_dispatcher()` and `has_dispatcher()` functions
   - Update `register()` to store dispatcher
   - ~30-40 lines of code changes

3. ✏️ **`ractor_shell/src/lib.rs`**
   - Add `dispatcher()` method to `SchemaProvider` trait
   - Export `RpcDispatcher` type
   - ~5-10 lines of code changes

4. ✏️ **`ractor_shell/src/introspection.rs`**
   - Update `call_typed_rpc_on_actor()` to use dispatcher
   - ~15-20 lines of code changes

### Testing

5. ➕ **`ractor_cluster_derive/src/tests.rs`** (new or existing)
   - Add macro expansion tests
   - ~50 lines

6. ➕ **`ractor_shell/tests/typed_rpc_dispatcher.rs`** (new)
   - Integration tests for dispatcher
   - ~100 lines

### No Changes Needed

- ✅ `ractor_shell/examples/cluster_demo/raft.rs` - Already calls `register::<RaftMessage>()`
- ✅ User code - Just needs `#[ractor_shell]` attribute (already present)

---

## Migration Path

### For Existing Actors with `#[ractor_shell]`

**No changes required!** Actors already using `#[ractor_shell]` will automatically get:
- Dispatcher generation
- Automatic registration when calling `schema_registry::register::<T>()`
- Remote RPC support

### For New Actors

Same as before - just add `#[ractor_shell]`:

```rust
#[derive(RactorClusterMessage)]
#[ractor_shell]  // ← This is all you need
pub enum MyMessage {
    #[rpc]
    DoSomething(RpcReplyPort<String>),
}
```

---

## Success Criteria

### Functional Requirements

- [ ] `call raft_node GetStatus {}` works from remote shell
- [ ] `call raft_node IsLeader {}` returns boolean
- [ ] `call raft_node GetLeader {}` returns Option<String>
- [ ] `call raft_node GetPeers {}` returns Vec<String>
- [ ] Timeouts are handled gracefully
- [ ] Works for both local and remote actors
- [ ] Error messages are clear and actionable

### Code Quality

- [ ] Macro-generated code is readable (test with `cargo expand`)
- [ ] No unsafe code
- [ ] Proper error handling (no unwraps in library code)
- [ ] Documentation for new types and functions
- [ ] Unit tests pass
- [ ] Integration tests pass

### Performance

- [ ] No measurable performance impact on actor message processing
- [ ] Dispatcher registration is one-time at actor startup
- [ ] Type-erased boxing is acceptable (only happens at shell RPC time)

---

## Rollout Plan

### Phase 1: Core Implementation (Day 1)
1. Implement macro changes
2. Update schema registry
3. Update introspection layer
4. Basic smoke testing

### Phase 2: Testing & Polish (Day 2)
1. Write comprehensive tests
2. Test with cluster_demo
3. Handle edge cases
4. Documentation updates

### Phase 3: Review & Merge (Day 3)
1. Code review
2. Address feedback
3. Update TASKS.md to mark as complete
4. Merge to main branch

---

## Future Enhancements (Out of Scope)

### Argument Parsing from JSON

Currently dispatchers ignore `args` parameter. Future enhancement could:

```rust
// User writes:
#[rpc]
UpdateValue(i32, RpcReplyPort<bool>)

// Macro generates:
"UpdateValue" => {
    let value: i32 = serde_json::from_value(args)
        .map_err(|e| format!("Invalid args: {}", e))?;
    let result = ractor::call!(actor_ref, |port|
        RaftMessage::UpdateValue(value, port)
    ).await?;
    Ok(serde_json::json!(result))
}
```

This requires:
- Parsing non-RpcReplyPort fields as arguments
- Deserializing JSON to those types
- Passing them to the message constructor

**Complexity**: Medium. Should be a separate PR after basic dispatcher works.

### Custom Timeout Configuration

Allow per-variant timeout configuration:

```rust
#[rpc(timeout_ms = 10000)]
SlowOperation(RpcReplyPort<String>)
```

### Batch RPC Support

Support multiple RPCs in one call for efficiency.

---

## Open Questions

1. **Should dispatchers be generated for Cast messages too?**
   - Currently only `#[rpc]` variants
   - Could add `#[cast]` support for fire-and-forget

2. **How to handle actors with no RPCs?**
   - Currently: `dispatcher()` returns `None`
   - Alternative: Don't generate dispatcher module at all

3. **Error message format?**
   - Currently: Plain strings
   - Alternative: Structured error types

**Recommendation**: Start simple (RPC-only, string errors), iterate based on feedback.

---

## Conclusion

This plan provides a clean, type-safe solution to the remote typed RPC problem without architectural compromises. The macro does the heavy lifting at compile time, generating dispatcher code that bridges the shell's JSON-based protocol with typed actor messages.

**Key advantages**:
- ✅ No runtime reflection
- ✅ Compile-time type safety
- ✅ Clean separation (library/example boundary)
- ✅ Automatic (via macro)
- ✅ Extensible (works for all `#[ractor_shell]` actors)

**Estimated total time**: 2-3 hours for experienced Rust developer familiar with the codebase.

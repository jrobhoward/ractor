# ractor_shell Integration Guide

**Date**: 2026-01-16
**Branch**: feature/shell

## What Was Integrated

The `ractor_shell` interactive REPL has been integrated into the ractor workspace as a conditional workspace member. This shell provides Erlang Observer-like capabilities for debugging and introspecting running actor systems.

## Integration Details

### Workspace Changes

1. **Added ractor_shell to workspace members** (`Cargo.toml`)
   - `ractor_shell` appears in the `members` list
   - NOT included in `default-members` (optional, not built by default)

2. **Updated ractor_shell dependencies** (`ractor_shell/Cargo.toml`)
   - Changed from crates.io versions to workspace paths:
     - `ractor = { path = "../ractor", features = ["cluster", "blanket_serde"] }`
     - `ractor_cluster = { path = "../ractor_cluster" }`

3. **Updated CLAUDE.md** with ractor_shell documentation

## How to Build

### Build ractor_shell Specifically

```bash
# Build the shell
cargo build -p ractor_shell

# Run shell examples
cargo run --example demo -p ractor_shell
cargo run --example dynamic_actor -p ractor_shell
cargo run --example monitoring_demo -p ractor_shell

# Install shell binary
cargo install --path ractor_shell
```

### Build Entire Workspace (Including Shell)

```bash
# Build everything
cargo build --workspace

# Build with examples
cargo build --workspace --examples
```

### Default Build (Excludes Shell)

```bash
# This will NOT build ractor_shell
cargo build

# Only builds: ractor, ractor_cluster, ractor_cluster_derive
```

## Why This Approach?

The shell is **not built by default** because:

1. **Optional Development Tool**: Most users won't need the interactive shell
2. **Additional Dependencies**: Shell has dependencies (rustyline, tabled, colored, etc.) not needed for core ractor
3. **Compilation Time**: Excluding it from default builds speeds up iteration
4. **Clean Separation**: Core library vs debugging tools

Users who want the shell can explicitly build it with `-p ractor_shell`.

## File Structure

```
ractor/
├── Cargo.toml                    # Modified: Added ractor_shell to members
├── CLAUDE.md                     # Modified: Added shell documentation
└── ractor_shell/                 # NEW: Complete ractor_shell crate
    ├── Cargo.toml                # Modified: Use workspace paths
    ├── README.md                 # Complete feature documentation
    ├── DYNAMIC_MESSAGES.md       # Dynamic message interface guide
    ├── MONITORING.md             # Monitoring feature guide
    ├── src/
    │   ├── lib.rs                # Shell state and command execution
    │   ├── main.rs               # CLI entry point
    │   ├── commands/             # Individual command implementations
    │   ├── dynamic.rs            # DynamicMessage interface
    │   ├── introspection.rs      # Remote introspection actor
    │   ├── monitor.rs            # Actor monitoring infrastructure
    │   └── completer.rs          # Tab completion support
    └── examples/
        ├── demo.rs               # Basic shell demo
        ├── dynamic_actor.rs      # Dynamic message example
        └── monitoring_demo.rs    # Monitoring example
```

## Features Included

### Core Shell Capabilities

- ✅ Actor enumeration (via registry and process groups)
- ✅ Actor introspection (status, ID, name)
- ✅ Process group queries
- ✅ Remote node connections (ractor_cluster)
- ✅ Dynamic JSON message sending
- ✅ Actor lifecycle monitoring
- ✅ Cluster topology visualization
- ✅ Tab completion and command aliases

### Dynamic Message Interface

Actors can opt-in to receive arbitrary JSON:

```rust
use ractor_shell::dynamic::DynamicMessage;

impl Actor for MyActor {
    type Msg = DynamicMessage;
    // Handle Cast(json), Call(json, reply), Ping(reply)
}
```

Shell usage:
```bash
ractor@local > send my_actor {"command": "do_something"}
ractor@local > call my_actor {"command": "get_status"}
```

### Current Limitations

- Only shows named/registered actors (requires future introspection APIs for all actors)
- Shows process groups instead of full supervision trees (requires core API enhancements)
- Real-time monitoring infrastructure exists but needs deeper ractor integration
- Remote message sending not yet fully implemented

See `ractor_shell/README.md` for complete documentation.

## Next Steps (Future)

This integration is **Phase 1** of the plan outlined in `FORK_INTEGRATION_PLAN.md`. Future phases will add:

### Phase 2: Core Introspection APIs (Future)

Add feature-gated APIs to ractor core:

```rust
#[cfg(feature = "shell")]
pub mod introspection {
    pub fn get_all_actors() -> Vec<ActorCell>;
    pub fn get_actor_info(cell: &ActorCell) -> ActorInfo;
    pub fn get_supervision_tree() -> SupervisionTree;
    pub fn subscribe_events() -> Receiver<SupervisionEvent>;
}
```

This would enable:
- Full actor enumeration (not just registered)
- True supervision tree visualization
- Real-time supervision event monitoring
- Message queue depth inspection

### Phase 3: Enhanced Shell (Future)

Once introspection APIs exist:
- Erlang `i/0` equivalent command (show all actors with stats)
- Full supervision tree visualization
- Real-time monitoring without polling
- Message queue inspection

## Testing

```bash
# Test ractor_shell
cargo test -p ractor_shell

# Run shell examples to verify
cargo run --example demo -p ractor_shell
cargo run --example dynamic_actor -p ractor_shell
cargo run --example monitoring_demo -p ractor_shell
```

## Migration from ractor_experiments

If you were using `ractor_experiments/ractor_shell`, update your dependencies:

```toml
# Old (ractor_experiments)
[dependencies]
ractor = "0.15"
ractor_shell = { path = "path/to/ractor_experiments/ractor_shell" }

# New (integrated fork)
[dependencies]
ractor = { git = "https://github.com/YOUR_ORG/ractor", branch = "feature/shell", features = ["cluster", "blanket_serde"] }
ractor_shell = { git = "https://github.com/YOUR_ORG/ractor", branch = "feature/shell" }
```

Or if using local path:
```toml
[dependencies]
ractor = { path = "../ractor/ractor", features = ["cluster", "blanket_serde"] }
ractor_shell = { path = "../ractor/ractor_shell" }
```

## Documentation

- **ractor_shell/README.md**: Complete shell feature documentation
- **ractor_shell/DYNAMIC_MESSAGES.md**: Guide for dynamic message interface
- **ractor_shell/MONITORING.md**: Actor monitoring guide
- **ractor_shell/UX_FEATURES.md**: Tab completion and aliases
- **FORK_INTEGRATION_PLAN.md**: Full integration roadmap (in ractor_experiments)

## Contributing

The shell is under active development. Contributions welcome!

Areas for contribution:
- Additional shell commands
- Improved introspection (when core APIs available)
- Better formatting and UX
- Remote capabilities
- Testing and documentation

See `CONTRIBUTING.md` in the root directory.

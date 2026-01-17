# Ractor Fork & Shell Integration Plan

**Date**: 2026-01-16
**Goal**: Fork https://github.com/slawlor/ractor and integrate ractor_shell as a workspace member with feature-gated introspection APIs

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Repository Setup](#repository-setup)
3. [Workspace Structure](#workspace-structure)
4. [Implementation Phases](#implementation-phases)
5. [Core API Design](#core-api-design)
6. [Migration Strategy](#migration-strategy)
7. [Testing Strategy](#testing-strategy)
8. [Maintenance & Upstreaming](#maintenance--upstreaming)
9. [Success Criteria](#success-criteria)

---

## Executive Summary

### Why Fork?

Current ractor_shell limitations blocked by missing core APIs:
- ✗ Cannot enumerate all actors (only registered ones)
- ✗ Cannot inspect message queue depths
- ✗ Cannot query actual supervision trees (only process groups)
- ✗ Cannot subscribe to real-time supervision events
- ✗ Cannot get detailed actor statistics (memory, reductions equivalent)

### What We'll Achieve

After integration:
- ✓ Full actor enumeration like Erlang's `i/0`
- ✓ Real-time monitoring like Erlang Observer
- ✓ True supervision tree introspection
- ✓ Message queue depth inspection
- ✓ Performance statistics per actor
- ✓ Zero runtime cost when shell feature disabled

### Timeline Estimate

- **Phase 1** (Foundation): 2-3 days
- **Phase 2** (Core APIs): 5-7 days
- **Phase 3** (Shell Integration): 3-4 days
- **Phase 4** (Polish & Docs): 2-3 days
- **Total**: 12-17 days (2-3 weeks)

---

## Repository Setup

### Step 1: Fork Creation

```bash
# Fork on GitHub
# Navigate to https://github.com/slawlor/ractor
# Click "Fork" → Create fork under your organization/account

# Clone your fork
git clone git@github.com:YOUR_ORG/ractor.git
cd ractor

# Add upstream remote for syncing
git remote add upstream https://github.com/slawlor/ractor.git
git fetch upstream

# Create development branch
git checkout -b feature/shell-integration
```

### Step 2: Branch Strategy

```
main (your fork)
  ↓
feature/shell-integration (work branch)
  ↓
  ├── feature/introspection-api (Phase 2)
  ├── feature/shell-workspace (Phase 3)
  └── feature/documentation (Phase 4)
```

**Workflow:**
- Keep `main` in sync with upstream for easy merging
- Do all work in feature branches
- Merge feature branches → `feature/shell-integration`
- Periodically merge `upstream/main` → `main` → `feature/shell-integration`

### Step 3: Tracking Upstream

```bash
# Weekly upstream sync
git checkout main
git fetch upstream
git merge upstream/main
git push origin main

# Merge into development branch
git checkout feature/shell-integration
git merge main
# Resolve conflicts if any
git push origin feature/shell-integration
```

---

## Workspace Structure

### Current Ractor Structure

```
ractor/
├── Cargo.toml (workspace root)
├── ractor/            (core actor framework)
├── ractor_cluster/    (distributed actors)
└── ractor_cluster_integration_tests/
```

### Proposed Structure After Integration

```
ractor/
├── Cargo.toml (workspace root - MODIFIED)
├── ractor/            (core - MODIFIED)
│   ├── src/
│   │   ├── lib.rs     (add introspection module export)
│   │   └── introspection.rs (NEW - feature-gated APIs)
│   └── Cargo.toml     (add shell feature)
├── ractor_cluster/    (no changes)
├── ractor_cluster_integration_tests/
└── ractor_shell/      (NEW - migrated from ractor_experiments)
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs
    │   ├── commands/
    │   ├── dynamic.rs
    │   ├── introspection.rs
    │   ├── monitor.rs
    │   └── completer.rs
    ├── examples/
    │   ├── demo.rs
    │   ├── dynamic_actor.rs
    │   └── monitoring_demo.rs
    └── README.md
```

### Workspace Cargo.toml Changes

```toml
[workspace]
members = [
    "ractor",
    "ractor_cluster",
    "ractor_cluster_integration_tests",
    "ractor_shell",  # NEW
]
resolver = "2"

[workspace.dependencies]
# Shared dependencies for consistency
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Shell-specific (only used by ractor_shell)
rustyline = "14"
tabled = "0.15"
colored = "2"
clap = { version = "4", features = ["derive"] }
```

### Feature Flag Design

**In `ractor/Cargo.toml`:**

```toml
[features]
default = []
cluster = ["ractor_cluster"]

# NEW: Shell introspection support
shell = [
    "dep:dashmap",  # Already a dependency, but make it explicit
]

# For development/debugging (includes all features)
full = ["cluster", "shell"]
```

**Philosophy:**
- `shell` feature adds ZERO runtime cost when disabled
- All introspection code is `#[cfg(feature = "shell")]`
- Production builds omit shell by default
- Development builds use `--features=full`

---

## Implementation Phases

### Phase 1: Foundation (2-3 days)

**Goal**: Set up workspace and identify internal structures

#### Tasks:

1. **Fork and Clone** (30 min)
   - Fork ractor repository
   - Clone locally
   - Set up upstream remote

2. **Add ractor_shell to Workspace** (2 hours)
   - Copy `ractor_experiments/ractor_shell/` → `ractor/ractor_shell/`
   - Update workspace `Cargo.toml`
   - Update `ractor_shell/Cargo.toml` dependencies to use workspace members
   - Verify builds: `cargo build -p ractor_shell`

3. **Code Audit** (1 day)
   - Study `ractor/src/actor/actor_cell.rs` - actor internal state
   - Study `ractor/src/registry.rs` - actor registry implementation
   - Study `ractor/src/pg.rs` - process group implementation
   - Identify supervision tree data structures
   - Map out message channel internals (queue depth access)

4. **Design Document** (4 hours)
   - Document current internal APIs
   - List specific fields/methods needed for introspection
   - Draft public API surface for `ractor::introspection`
   - Create ADR (Architecture Decision Record) for API design

**Deliverables:**
- ✓ Fork created with ractor_shell in workspace
- ✓ `cargo build --workspace` succeeds
- ✓ Internal code audit document
- ✓ API design document

---

### Phase 2: Core Introspection APIs (5-7 days)

**Goal**: Add feature-gated introspection module to ractor core

#### 2.1: Module Structure (1 day)

Create `ractor/src/introspection.rs`:

```rust
//! Introspection APIs for debugging and observability tools.
//!
//! This module is only available with the `shell` feature flag.
//! It provides low-level access to actor internals for tools like
//! ractor_shell, debuggers, and monitoring systems.
//!
//! **WARNING**: These APIs access internal actor state and should
//! only be used by trusted debugging tools, not production code.

#![cfg(feature = "shell")]

use crate::actor::{ActorCell, ActorId, ActorStatus};
use std::sync::Arc;

mod actor_info;
mod registry_query;
mod supervision_tree;
mod statistics;

pub use actor_info::{ActorInfo, MessageQueueInfo};
pub use registry_query::{get_all_actors, get_actor_by_id};
pub use supervision_tree::{SupervisionTree, SupervisionNode};
pub use statistics::{ActorStats, SystemStats};
```

#### 2.2: Actor Enumeration API (1 day)

**File**: `ractor/src/introspection/registry_query.rs`

```rust
use crate::ActorCell;
use dashmap::DashMap;
use std::sync::Arc;

/// Get all actors in the system (both registered and unregistered).
///
/// This function accesses internal actor tracking structures.
/// Returns a snapshot of all actors at call time.
pub fn get_all_actors() -> Vec<ActorCell> {
    // Implementation will access INTERNAL_ACTOR_REGISTRY
    // This is a new internal global we'll add to track ALL actors
    todo!("Access internal actor tracking")
}

/// Get all registered actors (name → ActorCell mapping).
pub fn get_registered_actors() -> Vec<(String, ActorCell)> {
    // Access existing registry::get_all() but return more details
    todo!()
}

/// Find actor by ID.
pub fn get_actor_by_id(id: &ActorId) -> Option<ActorCell> {
    // Search through internal tracking
    todo!()
}
```

**Required Internal Changes:**

In `ractor/src/actor/mod.rs` or `actor_cell.rs`:

```rust
#[cfg(feature = "shell")]
use once_cell::sync::Lazy;
#[cfg(feature = "shell")]
use dashmap::DashMap;

#[cfg(feature = "shell")]
static INTERNAL_ACTOR_REGISTRY: Lazy<DashMap<ActorId, ActorCell>> =
    Lazy::new(DashMap::new);

// In Actor::spawn() / spawn_linked():
#[cfg(feature = "shell")]
INTERNAL_ACTOR_REGISTRY.insert(actor_id.clone(), actor_cell.clone());

// In actor cleanup (when actor stops):
#[cfg(feature = "shell")]
INTERNAL_ACTOR_REGISTRY.remove(&actor_id);
```

#### 2.3: Actor Info API (1 day)

**File**: `ractor/src/introspection/actor_info.rs`

```rust
use crate::{ActorCell, ActorId, ActorStatus};

/// Detailed information about an actor's current state.
#[derive(Debug, Clone)]
pub struct ActorInfo {
    pub id: ActorId,
    pub name: Option<String>,
    pub status: ActorStatus,
    pub message_queue: MessageQueueInfo,
    pub supervisor: Option<ActorCell>,
    pub supervised_actors: Vec<ActorCell>,
}

/// Information about an actor's message queue.
#[derive(Debug, Clone)]
pub struct MessageQueueInfo {
    /// Number of pending messages
    pub depth: usize,
    /// Whether the actor is currently processing a message
    pub is_processing: bool,
}

/// Get comprehensive info about an actor.
pub fn get_actor_info(cell: &ActorCell) -> Option<ActorInfo> {
    // Access ActorCell internal fields
    // Need to expose or add methods to ActorCell:
    // - cell.get_message_queue_depth()
    // - cell.get_supervisor()
    // - cell.get_children()
    todo!("Implement via ActorCell internal access")
}
```

**Required ActorCell Changes:**

In `ractor/src/actor/actor_cell.rs`:

```rust
impl ActorCell {
    // ... existing methods ...

    #[cfg(feature = "shell")]
    pub fn get_message_queue_depth(&self) -> usize {
        // Access self.tx (message channel sender)
        // Use channel.len() or equivalent
        // May need to change channel type to support len()
        todo!()
    }

    #[cfg(feature = "shell")]
    pub fn get_supervisor(&self) -> Option<ActorCell> {
        // Access supervisor link if exists
        todo!()
    }

    #[cfg(feature = "shell")]
    pub fn get_supervised_actors(&self) -> Vec<ActorCell> {
        // Access children if this is a supervisor
        todo!()
    }
}
```

#### 2.4: Supervision Tree API (1-2 days)

**File**: `ractor/src/introspection/supervision_tree.rs`

```rust
use crate::{ActorCell, ActorId};

/// A node in the supervision tree.
#[derive(Debug, Clone)]
pub struct SupervisionNode {
    pub actor: ActorCell,
    pub children: Vec<SupervisionNode>,
}

/// The full supervision tree structure.
#[derive(Debug)]
pub struct SupervisionTree {
    pub roots: Vec<SupervisionNode>,
}

/// Build the complete supervision tree for the system.
pub fn get_supervision_tree() -> SupervisionTree {
    let all_actors = super::get_all_actors();

    // Algorithm:
    // 1. Build map of actor_id → ActorCell
    // 2. For each actor, get supervisor (if any)
    // 3. Build parent → children map
    // 4. Find roots (actors with no supervisor)
    // 5. Recursively build tree from roots

    todo!()
}

/// Build supervision subtree starting from specific actor.
pub fn get_supervision_subtree(root: &ActorCell) -> SupervisionNode {
    let children_cells = root.get_supervised_actors();
    let children = children_cells
        .into_iter()
        .map(|child| get_supervision_subtree(&child))
        .collect();

    SupervisionNode {
        actor: root.clone(),
        children,
    }
}
```

#### 2.5: Statistics API (1 day)

**File**: `ractor/src/introspection/statistics.rs`

```rust
use std::time::Duration;

/// Statistics for a single actor.
#[derive(Debug, Clone)]
pub struct ActorStats {
    /// Approximate number of messages processed (if tracked)
    pub messages_processed: Option<u64>,

    /// Current message queue depth
    pub queue_depth: usize,

    /// Time actor has been running
    pub uptime: Duration,

    /// Memory usage (if available via OS query)
    pub memory_bytes: Option<usize>,
}

/// System-wide statistics.
#[derive(Debug)]
pub struct SystemStats {
    pub total_actors: usize,
    pub running_actors: usize,
    pub stopped_actors: usize,
    pub total_queue_depth: usize,
}

pub fn get_actor_stats(cell: &ActorCell) -> ActorStats {
    todo!()
}

pub fn get_system_stats() -> SystemStats {
    let all_actors = super::get_all_actors();

    let total = all_actors.len();
    let running = all_actors.iter()
        .filter(|a| a.get_status() == ActorStatus::Running)
        .count();
    let stopped = total - running;
    let total_queue_depth = all_actors.iter()
        .map(|a| a.get_message_queue_depth())
        .sum();

    SystemStats {
        total_actors: total,
        running_actors: running,
        stopped_actors: stopped,
        total_queue_depth,
    }
}
```

#### 2.6: Event Subscription API (1-2 days)

**File**: `ractor/src/introspection/events.rs`

```rust
use crate::{ActorCell, ActorProcessingErr};
use tokio::sync::broadcast;

/// Supervision events that can be monitored.
#[derive(Debug, Clone)]
pub enum IntrospectionEvent {
    ActorStarted {
        actor: ActorCell,
    },
    ActorStopped {
        actor: ActorCell,
        reason: Option<String>,
    },
    ActorFailed {
        actor: ActorCell,
        error: String,
    },
    ActorPanicked {
        actor: ActorCell,
        panic_msg: String,
    },
}

/// Global event broadcaster for supervision events.
static EVENT_BROADCASTER: Lazy<broadcast::Sender<IntrospectionEvent>> =
    Lazy::new(|| {
        let (tx, _rx) = broadcast::channel(1000);
        tx
    });

/// Subscribe to supervision events.
///
/// Returns a receiver that will get all future supervision events.
/// The receiver has a buffer of 1000 events; older events are dropped.
pub fn subscribe_events() -> broadcast::Receiver<IntrospectionEvent> {
    EVENT_BROADCASTER.subscribe()
}

/// Internal function to broadcast an event (called by ractor core).
#[doc(hidden)]
pub fn broadcast_event(event: IntrospectionEvent) {
    let _ = EVENT_BROADCASTER.send(event);
}
```

**Required Integration Points:**

Add to actor lifecycle code:

```rust
// In Actor::spawn():
#[cfg(feature = "shell")]
crate::introspection::events::broadcast_event(
    IntrospectionEvent::ActorStarted { actor: cell.clone() }
);

// In actor stop handling:
#[cfg(feature = "shell")]
crate::introspection::events::broadcast_event(
    IntrospectionEvent::ActorStopped {
        actor: cell.clone(),
        reason: stop_reason.clone(),
    }
);

// In panic handler:
#[cfg(feature = "shell")]
crate::introspection::events::broadcast_event(
    IntrospectionEvent::ActorPanicked {
        actor: cell.clone(),
        panic_msg: panic_string.clone(),
    }
);
```

**Deliverables:**
- ✓ `ractor::introspection` module with all APIs
- ✓ Feature flag properly gates all code
- ✓ Internal actor tracking for enumeration
- ✓ ActorCell methods for queue depth, supervisor, children
- ✓ Event broadcast system integrated
- ✓ Unit tests for each API
- ✓ Documentation with examples

---

### Phase 3: Shell Integration (3-4 days)

**Goal**: Update ractor_shell to use new introspection APIs

#### 3.1: Dependency Updates (1 hour)

**In `ractor_shell/Cargo.toml`:**

```toml
[dependencies]
# Use workspace member with shell feature
ractor = { path = "../ractor", features = ["shell", "cluster"] }
ractor_cluster = { path = "../ractor_cluster" }

# ... rest of dependencies
```

#### 3.2: Update Shell Commands (2 days)

**actors command** - Use `get_all_actors()`:

```rust
// In ractor_shell/src/lib.rs or commands/actors.rs

use ractor::introspection::{get_all_actors, get_actor_info};

pub async fn execute_actors_command() -> Result<()> {
    let all_actors = get_all_actors();

    for actor_cell in all_actors {
        if let Some(info) = get_actor_info(&actor_cell) {
            println!("{:?} - {} - queue: {}",
                info.id,
                info.name.unwrap_or("unnamed".to_string()),
                info.message_queue.depth
            );
        }
    }
    Ok(())
}
```

**tree command** - Use `get_supervision_tree()`:

```rust
use ractor::introspection::get_supervision_tree;

pub async fn execute_tree_command() -> Result<()> {
    let tree = get_supervision_tree();

    for root in tree.roots {
        print_tree_node(&root, 0);
    }
    Ok(())
}

fn print_tree_node(node: &SupervisionNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let name = node.actor.get_name().unwrap_or("unnamed".to_string());
    println!("{}{} ({})", indent, name, node.actor.get_id());

    for child in &node.children {
        print_tree_node(child, depth + 1);
    }
}
```

**monitor command** - Use event subscription:

```rust
use ractor::introspection::{subscribe_events, IntrospectionEvent};

// In MonitorActor or shell state
pub async fn start_monitoring() -> Result<()> {
    let mut event_rx = subscribe_events();

    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                IntrospectionEvent::ActorStarted { actor } => {
                    println!("[STARTED] {}", actor.get_name().unwrap_or("unnamed"));
                }
                IntrospectionEvent::ActorStopped { actor, reason } => {
                    println!("[STOPPED] {} - {:?}",
                        actor.get_name().unwrap_or("unnamed"),
                        reason
                    );
                }
                IntrospectionEvent::ActorFailed { actor, error } => {
                    println!("[FAILED] {} - {}",
                        actor.get_name().unwrap_or("unnamed"),
                        error
                    );
                }
                IntrospectionEvent::ActorPanicked { actor, panic_msg } => {
                    println!("[PANIC] {} - {}",
                        actor.get_name().unwrap_or("unnamed"),
                        panic_msg
                    );
                }
            }
        }
    });

    Ok(())
}
```

**New: `i` command** - Erlang-style info dump:

```rust
use ractor::introspection::{get_all_actors, get_actor_info};
use tabled::{Table, Tabled};

#[derive(Tabled)]
struct ActorRow {
    id: String,
    name: String,
    status: String,
    queue: usize,
    supervisor: String,
    children: usize,
}

pub async fn execute_i_command() -> Result<()> {
    let all_actors = get_all_actors();

    let mut rows = Vec::new();
    for actor_cell in all_actors {
        if let Some(info) = get_actor_info(&actor_cell) {
            rows.push(ActorRow {
                id: format!("{}", info.id),
                name: info.name.unwrap_or("unnamed".to_string()),
                status: format!("{:?}", info.status),
                queue: info.message_queue.depth,
                supervisor: info.supervisor
                    .and_then(|s| s.get_name())
                    .unwrap_or("none".to_string()),
                children: info.supervised_actors.len(),
            });
        }
    }

    let table = Table::new(rows).to_string();
    println!("{}", table);
    Ok(())
}
```

#### 3.3: Update Documentation (1 day)

Update these files:
- `ractor_shell/README.md` - Add new commands, remove limitation notes
- `ractor_shell/MONITORING.md` - Document real-time events
- `ractor_shell/COMPLETION_GUIDE.md` - Update for new commands
- Create `ractor_shell/INTROSPECTION_API.md` - Guide for using core APIs

**Deliverables:**
- ✓ Shell commands use introspection APIs
- ✓ All TODOs resolved
- ✓ New `i` command implemented
- ✓ Real-time monitoring works
- ✓ Examples updated
- ✓ Documentation updated

---

### Phase 4: Polish & Documentation (2-3 days)

#### 4.1: Testing (1 day)

**Unit Tests** - In `ractor/src/introspection/`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Actor;

    #[tokio::test]
    async fn test_get_all_actors() {
        // Spawn some test actors
        let (actor1, _) = Actor::spawn(Some("test1".to_string()), TestActor, ()).await.unwrap();
        let (actor2, _) = Actor::spawn(Some("test2".to_string()), TestActor, ()).await.unwrap();

        // Query all actors
        let all_actors = get_all_actors();

        assert!(all_actors.len() >= 2);
        assert!(all_actors.iter().any(|a| a.get_name() == Some("test1".to_string())));
    }

    #[tokio::test]
    async fn test_supervision_tree() {
        // Create supervision hierarchy
        let (parent, _) = Actor::spawn(Some("parent".to_string()), SupervisorActor, ()).await.unwrap();
        // ... spawn children with parent as supervisor

        let tree = get_supervision_tree();
        assert!(tree.roots.len() > 0);
    }
}
```

**Integration Tests** - In `ractor_shell/tests/`:

```rust
#[tokio::test]
async fn test_shell_i_command() {
    // Spawn test actors
    // Execute i command
    // Verify output
}

#[tokio::test]
async fn test_real_time_monitoring() {
    // Subscribe to events
    // Spawn and stop actors
    // Verify events received
}
```

#### 4.2: Examples (1 day)

Create `ractor/examples/introspection_demo.rs`:

```rust
//! Demonstrates the introspection API for debugging actors.

use ractor::introspection::*;
use ractor::{Actor, ActorProcessingErr, ActorRef};

struct DemoActor;

impl Actor for DemoActor {
    type Msg = String;
    type State = ();
    type Arguments = ();

    async fn pre_start(&self, _: ActorRef<Self::Msg>, _: ())
        -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Spawn some actors
    let (actor1, _) = Actor::spawn(Some("demo1".to_string()), DemoActor, ()).await?;
    let (actor2, _) = Actor::spawn(Some("demo2".to_string()), DemoActor, ()).await?;

    // Get all actors
    println!("=== All Actors ===");
    for actor in get_all_actors() {
        if let Some(info) = get_actor_info(&actor) {
            println!("{:?}: {:?} (queue: {})",
                info.name, info.status, info.message_queue.depth);
        }
    }

    // Show supervision tree
    println!("\n=== Supervision Tree ===");
    let tree = get_supervision_tree();
    for root in tree.roots {
        print_node(&root, 0);
    }

    // Subscribe to events
    println!("\n=== Monitoring Events ===");
    let mut events = subscribe_events();

    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("Event: {:?}", event);
        }
    });

    // Stop actors to generate events
    actor1.stop(None);
    actor2.stop(None);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(())
}

fn print_node(node: &SupervisionNode, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{}{:?}", indent, node.actor.get_name());
    for child in &node.children {
        print_node(child, depth + 1);
    }
}
```

#### 4.3: Documentation (1 day)

**Create `ractor/INTROSPECTION.md`:**

```markdown
# Introspection API Guide

The introspection module provides debugging and observability tools...

## Feature Flag

Add to your `Cargo.toml`:
```toml
ractor = { version = "0.x", features = ["shell"] }
```

## API Overview

### Actor Enumeration
...

### Supervision Trees
...

### Real-time Events
...

## Security Considerations

The introspection API exposes internal actor state...
```

**Update `ractor/README.md`:**

Add section:
```markdown
## Debugging & Introspection

Ractor includes optional introspection APIs for debugging tools:

```toml
ractor = { features = ["shell"] }
```

See [INTROSPECTION.md](INTROSPECTION.md) for details.

### ractor_shell

Interactive REPL for observing running actor systems...
```

**Update `ractor_shell/README.md`:**

Remove "Current Limitations" section, add:
```markdown
## Requirements

ractor_shell requires ractor with the `shell` feature:

```toml
ractor = { features = ["shell", "cluster"] }
```

This enables full introspection capabilities.
```

**Deliverables:**
- ✓ Comprehensive test coverage
- ✓ Working examples
- ✓ Complete documentation
- ✓ Migration guide for users

---

## Core API Design

### Design Principles

1. **Feature-Gated**: All introspection code behind `#[cfg(feature = "shell")]`
2. **Zero-Cost**: No runtime overhead when feature disabled
3. **Safe**: Read-only access to internal state (no mutation)
4. **Snapshot Semantics**: All queries return point-in-time snapshots
5. **Non-Blocking**: Queries don't block actor processing

### API Surface

```rust
pub mod introspection {
    // Actor enumeration
    pub fn get_all_actors() -> Vec<ActorCell>;
    pub fn get_registered_actors() -> Vec<(String, ActorCell)>;
    pub fn get_actor_by_id(id: &ActorId) -> Option<ActorCell>;

    // Actor information
    pub fn get_actor_info(cell: &ActorCell) -> Option<ActorInfo>;
    pub struct ActorInfo {
        pub id: ActorId,
        pub name: Option<String>,
        pub status: ActorStatus,
        pub message_queue: MessageQueueInfo,
        pub supervisor: Option<ActorCell>,
        pub supervised_actors: Vec<ActorCell>,
    }

    // Supervision trees
    pub fn get_supervision_tree() -> SupervisionTree;
    pub fn get_supervision_subtree(root: &ActorCell) -> SupervisionNode;
    pub struct SupervisionTree { pub roots: Vec<SupervisionNode> }
    pub struct SupervisionNode {
        pub actor: ActorCell,
        pub children: Vec<SupervisionNode>,
    }

    // Statistics
    pub fn get_actor_stats(cell: &ActorCell) -> ActorStats;
    pub fn get_system_stats() -> SystemStats;

    // Event monitoring
    pub fn subscribe_events() -> broadcast::Receiver<IntrospectionEvent>;
    pub enum IntrospectionEvent {
        ActorStarted { actor: ActorCell },
        ActorStopped { actor: ActorCell, reason: Option<String> },
        ActorFailed { actor: ActorCell, error: String },
        ActorPanicked { actor: ActorCell, panic_msg: String },
    }
}
```

### Internal Changes Required

**In `ractor/src/actor/actor_cell.rs`:**

```rust
// Add global actor tracking
#[cfg(feature = "shell")]
static INTERNAL_ACTOR_REGISTRY: Lazy<DashMap<ActorId, ActorCell>> =
    Lazy::new(DashMap::new);

impl ActorCell {
    // Add introspection methods
    #[cfg(feature = "shell")]
    pub fn get_message_queue_depth(&self) -> usize { ... }

    #[cfg(feature = "shell")]
    pub fn get_supervisor(&self) -> Option<ActorCell> { ... }

    #[cfg(feature = "shell")]
    pub fn get_supervised_actors(&self) -> Vec<ActorCell> { ... }
}
```

**In actor lifecycle (spawn/stop/panic):**

```rust
#[cfg(feature = "shell")]
{
    INTERNAL_ACTOR_REGISTRY.insert(id.clone(), cell.clone());
    crate::introspection::broadcast_event(
        IntrospectionEvent::ActorStarted { actor: cell.clone() }
    );
}
```

---

## Migration Strategy

### For ractor_experiments Users

**Step 1**: Update dependencies

```diff
 [dependencies]
-ractor = "0.15"
+ractor = { git = "https://github.com/YOUR_ORG/ractor", branch = "feature/shell-integration", features = ["shell", "cluster"] }
```

**Step 2**: Remove ractor_shell submodule

```diff
 [dependencies]
-ractor_shell = { path = "ractor_shell" }
+ractor_shell = { git = "https://github.com/YOUR_ORG/ractor", branch = "feature/shell-integration" }
```

**Step 3**: Update imports (if using shell internals)

```diff
-use ractor_shell::ShellState;
+use ractor_shell::ShellState;  // No change needed

+// New: Direct access to introspection
+use ractor::introspection::{get_all_actors, subscribe_events};
```

### For New Users

```toml
[dependencies]
ractor = { git = "https://github.com/YOUR_ORG/ractor", features = ["shell"] }
ractor_shell = { git = "https://github.com/YOUR_ORG/ractor" }
```

### Backwards Compatibility

- All existing ractor code works unchanged
- Shell feature is opt-in
- No breaking changes to public API

---

## Testing Strategy

### Unit Tests

**Location**: `ractor/src/introspection/tests.rs`

**Coverage**:
- Actor enumeration with/without registered names
- Supervision tree construction
- Message queue depth tracking
- Event broadcasting
- Feature flag compilation (with/without shell)

**Example**:

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn enumerate_all_actors() {
        let actors = spawn_test_actors(10).await;
        let found = get_all_actors();
        assert_eq!(found.len(), 10);
    }

    #[tokio::test]
    async fn supervision_tree_hierarchy() {
        let tree = create_test_supervisor_tree().await;
        let introspection_tree = get_supervision_tree();
        verify_tree_structure(introspection_tree, tree);
    }
}
```

### Integration Tests

**Location**: `ractor_shell/tests/integration_tests.rs`

**Coverage**:
- Shell commands with introspection APIs
- Remote introspection via IntrospectionActor
- Event monitoring in live shell
- Cluster-wide introspection

### Performance Tests

**Location**: `ractor/benches/introspection_bench.rs`

**Benchmarks**:
- `get_all_actors()` with 1k, 10k, 100k actors
- `get_supervision_tree()` with deep hierarchies
- Event broadcast overhead
- Memory overhead of actor tracking

**Acceptance Criteria**:
- `get_all_actors()` < 10ms for 10k actors
- Event broadcast < 1μs per actor
- Memory overhead < 100 bytes per actor

### Continuous Integration

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
      - run: cargo test --workspace
      - run: cargo test --workspace --features shell
      - run: cargo test --workspace --all-features

  test-no-default-features:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - run: cargo test -p ractor --no-default-features

  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - run: cargo bench --features shell
```

---

## Maintenance & Upstreaming

### Keeping in Sync with Upstream

**Weekly Sync Process**:

```bash
#!/bin/bash
# sync-upstream.sh

set -e

echo "Fetching upstream ractor..."
git fetch upstream

echo "Checking for conflicts..."
git checkout main
git merge upstream/main --no-commit --no-ff

if [ $? -eq 0 ]; then
    echo "No conflicts, merging..."
    git merge --continue
    git push origin main

    echo "Merging to development branch..."
    git checkout feature/shell-integration
    git merge main
    git push origin feature/shell-integration
else
    echo "Conflicts detected. Resolve manually."
    exit 1
fi
```

**Automated Tracking**:

```yaml
# .github/workflows/upstream-sync.yml
name: Upstream Sync

on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly on Sunday
  workflow_dispatch:

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - run: |
          git remote add upstream https://github.com/slawlor/ractor.git
          git fetch upstream
          git checkout main
          git merge upstream/main
          git push origin main
```

### Upstreaming Strategy

**Goal**: Contribute introspection APIs back to upstream ractor

**Phase 1: RFC/Discussion** (Before implementation)
- Open GitHub issue on upstream ractor
- Title: "RFC: Introspection API for debugging tools"
- Present use case (ractor_shell)
- Show draft API design
- Solicit feedback from maintainer

**Phase 2: Minimal PR** (After Phase 2 completion)
- Create PR with just the introspection module
- No shell integration yet
- Focus on core value: debugging APIs
- Comprehensive tests and docs
- Demonstrate zero runtime cost

**Phase 3: Shell PR** (After Phase 3 completion)
- Separate PR adding ractor_shell to workspace
- Reference introspection PR
- Show how shell uses the APIs
- Position as "official" debugging tool

**Phase 4: Maintenance**
- Offer to co-maintain introspection module
- Respond to issues/PRs quickly
- Keep documentation updated

**If Upstreaming Fails**:
- Maintain fork independently
- Tag releases in sync with upstream versions
- Provide migration guide for users
- Consider publishing to crates.io as `ractor-introspection`

### Release Strategy

**Versioning**:
- Track upstream ractor version
- Fork version: `<upstream_version>+shell.<increment>`
- Example: `0.15.0+shell.1`, `0.15.0+shell.2`

**Release Process**:

```bash
# After merging upstream 0.15.1
git tag v0.15.1+shell.1
git push origin v0.15.1+shell.1

# Create GitHub release
gh release create v0.15.1+shell.1 \
  --title "Ractor 0.15.1 + Shell Integration" \
  --notes "Based on upstream ractor 0.15.1 with introspection APIs and ractor_shell"
```

**Publishing to crates.io**:

```toml
# If upstream doesn't accept PRs, publish separately
[package]
name = "ractor-shell"  # Don't conflict with "ractor"
version = "0.1.0"
```

---

## Success Criteria

### Must Have (MVP)

- ✓ Fork exists with ractor_shell in workspace
- ✓ `cargo build --workspace` succeeds with and without `shell` feature
- ✓ `get_all_actors()` returns all actors (not just registered)
- ✓ `get_actor_info()` includes message queue depth
- ✓ `get_supervision_tree()` shows actual parent/child relationships
- ✓ Real-time event monitoring works
- ✓ Shell `actors`, `tree`, `monitor` commands use new APIs
- ✓ Zero runtime overhead when `shell` feature disabled
- ✓ Documentation complete

### Should Have (Nice to Have)

- ✓ `i` command (Erlang-style info dump)
- ✓ Actor statistics (memory, uptime)
- ✓ Remote introspection (cluster-wide queries)
- ✓ Performance benchmarks
- ✓ Integration tests for all shell commands
- ✓ Migration guide from ractor_experiments

### Could Have (Future Work)

- ○ Graphical UI (Observer-like)
- ○ Trace integration (add actors to trace from shell)
- ○ SASL log integration (per-actor logs)
- ○ Actor spawning from shell
- ○ Hot code reload experiments
- ○ WebSocket API for remote shells

---

## Timeline & Milestones

### Week 1

**Days 1-2**: Phase 1 (Foundation)
- Fork repository
- Add ractor_shell to workspace
- Code audit

**Days 3-5**: Phase 2 Start (Core APIs)
- Create introspection module structure
- Implement actor enumeration
- Implement actor info API

### Week 2

**Days 6-8**: Phase 2 Complete
- Implement supervision tree API
- Implement statistics API
- Implement event subscription
- Write tests

**Days 9-10**: Phase 3 Start (Shell Integration)
- Update shell dependencies
- Migrate shell commands to use new APIs

### Week 3

**Days 11-12**: Phase 3 Complete
- Implement new `i` command
- Update all documentation
- Write integration tests

**Days 13-15**: Phase 4 (Polish)
- Complete test coverage
- Write examples
- Final documentation review
- Prepare upstream RFC

**Day 15**: Release v0.1.0+shell.1

---

## Risk Management

### Risk: Upstream Breaking Changes

**Likelihood**: Medium
**Impact**: High

**Mitigation**:
- Monitor upstream ractor issues/PRs
- Set up automated upstream sync checks
- Maintain compatibility layer if needed
- Document breaking changes in fork

### Risk: Performance Regression

**Likelihood**: Low
**Impact**: High

**Mitigation**:
- Comprehensive benchmarks before/after
- Feature flag ensures zero cost when disabled
- Profile introspection queries
- Lazy initialization of tracking structures

### Risk: Upstream Rejects PRs

**Likelihood**: Medium
**Impact**: Medium

**Mitigation**:
- Early RFC discussion
- Show clear use case value
- Offer to co-maintain
- Prepare for independent maintenance
- Consider publishing as separate crate

### Risk: API Instability

**Likelihood**: Medium
**Impact**: Low

**Mitigation**:
- Mark introspection APIs as unstable initially
- Gather user feedback before stabilizing
- Version introspection module separately if needed

---

## Appendix A: File Checklist

### New Files to Create

```
ractor/
├── src/
│   └── introspection/
│       ├── mod.rs
│       ├── actor_info.rs
│       ├── registry_query.rs
│       ├── supervision_tree.rs
│       ├── statistics.rs
│       ├── events.rs
│       └── tests.rs
├── examples/
│   └── introspection_demo.rs
├── benches/
│   └── introspection_bench.rs
├── INTROSPECTION.md
└── FORK_MAINTENANCE.md

ractor_shell/
├── tests/
│   └── integration_tests.rs
└── INTROSPECTION_API.md
```

### Files to Modify

```
ractor/
├── Cargo.toml (add workspace member, features)
├── src/lib.rs (export introspection module)
├── src/actor/actor_cell.rs (add introspection methods)
├── src/actor/mod.rs (add actor tracking, event broadcast)
└── README.md (add introspection section)

ractor_shell/
├── Cargo.toml (update dependencies)
├── src/lib.rs (use introspection APIs)
├── src/commands/*.rs (update command implementations)
├── README.md (update features, remove limitations)
├── MONITORING.md (document real-time events)
└── examples/*.rs (update to use new APIs)

.github/workflows/
├── ci.yml (add shell feature tests)
└── upstream-sync.yml (NEW - automated sync)
```

---

## Appendix B: Communication Plan

### Upstream Communication

**Initial Contact**:
```markdown
Title: RFC: Introspection API for debugging tools

Hi! I've been working on an Erlang-style REPL for ractor called ractor_shell.
It's inspired by Erlang's Observer and provides interactive debugging.

Currently, it's limited by lack of internal API access:
- Can only see registered actors (not all actors)
- Can't inspect message queue depths
- Can't query actual supervision trees
- Can't subscribe to supervision events

I'm proposing to add an optional `introspection` module (feature-gated) that
would expose these capabilities in a safe, read-only way.

Would you be interested in this? I'm happy to implement it and submit a PR.

Design doc: [link to this plan]
Working prototype: [link to ractor_experiments]
```

### User Communication

**Announcement** (after Phase 4):
```markdown
Title: ractor_shell: Interactive REPL for Ractor now available!

We've forked ractor to add introspection APIs and integrated ractor_shell
as a workspace member. This provides Erlang Observer-like capabilities:

Features:
- Interactive REPL for inspecting running actors
- Full actor enumeration (not just registered)
- Supervision tree visualization
- Real-time event monitoring
- Message queue inspection
- Cluster-wide introspection

Get started:
```toml
ractor = { git = "https://github.com/YOUR_ORG/ractor", features = ["shell"] }
ractor_shell = { git = "https://github.com/YOUR_ORG/ractor" }
```

See README for full documentation.

Note: We're working to upstream these changes to the main ractor repository.
```

---

## Next Steps

1. **Create fork** on GitHub
2. **Clone locally** and set up remotes
3. **Create feature branch**: `feature/shell-integration`
4. **Begin Phase 1**: Add ractor_shell to workspace
5. **Schedule weekly upstream syncs**
6. **Open upstream RFC** for early feedback
7. **Execute phases** according to timeline
8. **Document progress** in GitHub project board

---

**Document Version**: 1.0
**Last Updated**: 2026-01-16
**Author**: [Your Name]
**Status**: Ready for Implementation

# Phase 2 Testing Guide - Introspection & Display

This guide shows how to test the enhanced introspection and system statistics features.

## What's New in Phase 2

Phase 2 adds deeper system visibility:
- **`stats` command** - System-wide statistics (actor counts, process groups, etc.)
- **`tree` command** - Visual tree of process group organization
- **Enhanced displays** - Better formatting for system information

## Prerequisites

- Build the project: `cargo build --all`
- Three terminal windows (for cluster testing)

## Test Procedure

### Terminal 1: Start node_b

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments
cargo run --bin node_b
```

### Terminal 2: Start node_a

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments
cargo run --bin node_a
```

###Terminal 3: Start the Shell

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments/ractor_shell
cargo run
```

## Shell Test Commands

### 1. Local Statistics

Before connecting to any nodes, check local stats:

```
ractor@local > stats
```

Expected output:
```
System Statistics

Node: local

  Registered Actors: 0

  Process Groups: 0
  Actors in Groups: 0
```

### 2. Connect and View Remote Stats

```
ractor@local > connect 127.0.0.1:9002
ractor@local > use 127.0.0.1:9002
ractor@127.0.0.1:9002 > stats
```

Expected output:
```
System Statistics

Node: 127.0.0.1:9002

  Registered Actors: 2
    Running 2

  Cluster Nodes: 2
  Process Groups: 2
  Actors in Groups: 4
```

### 3. Local Process Group Tree

```
ractor@127.0.0.1:9002 > use local
ractor@local > tree
```

Expected output:
```
Process Group Tree

Note: Full supervision tree introspection requires ractor core API enhancements.
      Showing process group organization as a simplified tree.

├── ping_pong
│   ├── ping_pong (1.0)
│   └── ping_pong (2.0)

├── ractor_shell_introspection
│   ├── introspection (1.1)
│   └── introspection (2.1)

Future Enhancement:
  Once ractor exposes supervision tree APIs, this command will show:
  - True parent/child actor relationships
  - Supervisor strategies
  - Actor restart counts
```

### 4. Cluster-Wide Tree

After connecting to a node and discovering topology:

```
ractor@local > connect 127.0.0.1:9002
ractor@local > tree
```

Expected output:
```
Process Group Tree

Note: Full supervision tree introspection requires ractor core API enhancements.
      Showing process group organization as a simplified tree.

├── ping_pong
│   ├── ping_pong (1.0) on node_b
│   └── ping_pong (2.0) on node_a

├── ractor_shell_introspection
│   ├── introspection (1.1) on node_b
│   └── introspection (2.1) on node_a

Future Enhancement:
  Once ractor exposes supervision tree APIs, this command will show:
  - True parent/child actor relationships
  - Supervisor strategies
  - Actor restart counts
```

### 5. Compare Stats Across Nodes

Switch between nodes and compare statistics:

```
ractor@local > connect 127.0.0.1:9002
ractor@local > connect 127.0.0.1:9001
ractor@local > use 127.0.0.1:9002
ractor@127.0.0.1:9002 > stats
ractor@127.0.0.1:9002 > use 127.0.0.1:9001
ractor@127.0.0.1:9001 > stats
ractor@127.0.0.1:9001 > use local
ractor@local > stats
```

## Key Features Demonstrated

### System Statistics (`stats`)
- Shows total registered actors
- Breaks down actors by status (Running, Stopped, etc.)
- Displays process group counts
- Shows cluster information when available
- Works both locally and remotely

### Process Group Tree (`tree`)
- Visual tree representation of actor organization
- Shows actors grouped by process group
- Displays node information in cluster mode
- Clear indication that true supervision trees require ractor enhancements

### Enhanced Info Display
- The existing `info` command already shows detailed actor information
- Stats command aggregates this across the system
- Tree command provides structural overview

## Limitations & Future Enhancements

### Current Limitations

1. **No True Supervision Trees**
   - Ractor 0.15 doesn't expose parent/child relationships
   - `tree` shows process groups instead
   - Can't show supervisor strategies or restart policies

2. **Known Process Groups Only**
   - Tree command checks well-known groups
   - May miss custom process groups
   - Future: enumerate all groups

3. **Actor Status Only**
   - Stats shows actor counts and statuses
   - No message queue depths or processing rates
   - Future: more detailed metrics

### Future Enhancements

Once ractor core adds supervision introspection APIs:

1. **True Supervision Trees**
   ```
   ├── my_supervisor (Strategy: OneForOne)
   │   ├── worker_1 (Restarts: 0)
   │   ├── worker_2 (Restarts: 2)
   │   └── sub_supervisor (Strategy: OneForAll)
   │       ├── worker_3 (Restarts: 0)
   │       └── worker_4 (Restarts: 0)
   ```

2. **Detailed Actor Metrics**
   ```
   System Statistics:
     Registered Actors: 15
       Running: 14
       Stopping: 1
     Message Processing:
       Total messages: 45,231
       Avg processing time: 2.3ms
       Queue depths: [0, 0, 3, 1, 0, ...]
   ```

3. **Custom Introspectable Trait**
   ```rust
   impl Introspectable for MyActor {
       fn describe_state(state: &Self::State) -> String {
           format!("Counter: {}, Mode: {:?}", state.count, state.mode)
       }
   }
   ```

## Phase 2 Complete!

You've successfully:
- ✓ Viewed system statistics with `stats`
- ✓ Visualized process group organization with `tree`
- ✓ Seen how stats work locally and remotely
- ✓ Understood the current limitations and future roadmap

Next phases:
- Phase 3: Live monitoring and event streams
- Phase 4: Message construction and sending
- Phase 7: Tab completion and UX polish

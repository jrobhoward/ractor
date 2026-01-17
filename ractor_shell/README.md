# Ractor Shell

Interactive REPL for Ractor actor systems, inspired by Erlang's `erl` shell.

## Status: Phases 2, 3, 4, 5, 6 & 7 Complete ✓

Full cluster topology awareness, enhanced introspection, file-based message/script input, actor lifecycle monitoring, and delightful UX with tab completion and aliases.

### Implemented Commands

| Command | Description | Status |
|---------|-------------|--------|
| `help [command]` | Show help/usage | ✅ |
| `actors` | List all named actors (local or remote) | ✅ |
| `registry` | Show registered actors (local or remote) | ✅ |
| `pg list` | List process groups (with note) | ✅ |
| `pg members <group>` | Show process group members (local or remote) | ✅ |
| `info <actor>` | Show actor details (local or remote) | ✅ |
| `send <actor> <msg>` | Send cast message (JSON) | ✅ Dynamic actors |
| `call <actor> <msg>` | Send RPC message (JSON) | ✅ Dynamic actors |
| `send-file <actor> <file>` | Send message from JSON file | ✅ Dynamic actors |
| `load <script>` | Execute commands from script file | ✅ |
| `stop <actor>` | Stop an actor | ✅ |
| `connect <host:port>` | Connect to remote node | ✅ |
| `disconnect <node>` | Disconnect from node | ✅ |
| `nodes` | List connected nodes | ✅ |
| `use <node>` | Switch to node context | ✅ |
| `cluster` | Show cluster topology | ✅ |
| `cluster nodes` | Show all nodes in cluster | ✅ |
| `cluster groups` | Show process groups across cluster | ✅ |
| `cluster actors` | Show all actors across cluster | ✅ |
| `stats` | Show system statistics | ✅ |
| `tree` | Show process group tree | ✅ |
| `monitor <actor>` | Start monitoring actor events | ✅ |
| `unmonitor <actor>` | Stop monitoring an actor | ✅ |
| `monitors` | List monitored actors | ✅ |
| `exit` / `quit` | Exit shell | ✅ |

## Quick Start

### Run the Demo

```bash
cargo run --example demo
```

This spawns 3 demo actors and starts the shell. Try:

```
ractor@local > registry
ractor@local > actors
ractor@local > pg members demo_group
ractor@local > info demo_actor_1
ractor@local > stop demo_actor_1
ractor@local > exit
```

### Use in Your Code

```rust
use ractor_shell::{ShellCommand, ShellState};
use rustyline::DefaultEditor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Spawn your actors...
    let (actor, _) = Actor::spawn(
        Some("my_actor".to_string()),
        MyActor,
        (),
    ).await?;

    // Start the shell
    let mut state = ShellState::new().await?;
    let mut rl = DefaultEditor::new()?;

    loop {
        let prompt = state.build_prompt();
        match rl.readline(&prompt) {
            Ok(line) => {
                if let Ok(cmd) = ShellCommand::parse_line(&line) {
                    state.execute(cmd).await?;
                }
                if state.should_exit {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}
```

## Features

### ✅ Implemented (Phases 1, 2, 4, 5, 6, 7)

- **Rich help system** with per-command documentation
- **Actor listing** from registry (named actors only)
- **Process group querying** to find actors by group membership
- **Actor introspection** showing ID, status, and name
- **Actor control** with stop command
- **Colored output** for better readability
- **Table formatting** for structured data
- **Command history** with readline support
- **Error handling** with helpful messages
- **Remote connection** to ractor_cluster nodes
- **Node context switching** with `use` command
- **Remote introspection** via IntrospectionActor and RPC
- **Multi-node management** with connect/disconnect
- **Automatic topology discovery** on connect
- **Cluster-wide visibility** of nodes, actors, and process groups
- **Mesh topology commands** (`cluster`, `cluster groups`, `cluster actors`)
- **Topology caching** for quick access
- **System statistics** (`stats` command) showing actor counts and statuses
- **Process group tree** (`tree` command) with visual hierarchy
- **Both local and remote stats** support
- **JSON message parsing** for send and call commands
- **File-based message input** with `send-file` command
- **Shell script execution** with `load` command
- **Dynamic message interface** - Actors using `DynamicMessage` can receive JSON from the shell
- **Actual message sending** - send/call commands work for dynamic actors
- **Educational explanations** for actors that don't support dynamic messages
- **Tab completion** - Context-aware completion for commands, actors, process groups, and nodes
- **Command aliases** - Short forms like `a` for `actors`, `r` for `registry`, `s` for `send`
- **Smart hints** - Inline hints showing alias expansions
- **Persistent command history** - Navigate previous commands with arrow keys
- **Actor monitoring** - Track lifecycle events (start, stop, panic, kill) for specific actors
- **Monitor management** - Start/stop monitoring actors, list currently monitored actors
- **Colored event display** - Visual formatting for different event types with timestamps

### 🚧 Current Limitations

- **Named actors only**: Full enumeration requires ractor core API additions
- **Process groups instead of supervision trees**: True supervision tree introspection requires ractor core APIs
- **Dynamic messages opt-in**: Actors must use `DynamicMessage` as their message type to receive JSON from the shell
- **Monitoring events pending**: Monitor infrastructure in place, real-time event display needs deeper ractor supervision integration

## Output Examples

### Registry Listing
```
ractor@local > registry
Registered Actors (3):
  demo_actor_1 → 0.0
  demo_actor_2 → 0.1
  demo_actor_3 → 0.2
```

### Actor Table
```
ractor@local > actors
+--------------+-----+---------+
| Name         | ID  | Status  |
+--------------+-----+---------+
| demo_actor_1 | 0.0 | Running |
| demo_actor_2 | 0.1 | Running |
| demo_actor_3 | 0.2 | Running |
+--------------+-----+---------+
```

### Process Group Members
```
ractor@local > pg members demo_group
+----------+--------------+-------+
| Actor ID | Name         | Local |
+----------+--------------+-------+
| 0.0      | demo_actor_1 | yes   |
| 0.1      | demo_actor_2 | yes   |
| 0.2      | demo_actor_3 | yes   |
+----------+--------------+-------+
```

### Actor Details
```
ractor@local > info demo_actor_1
Actor: demo_actor_1
  ID:     0.0
  Status: Running
  Name:   demo_actor_1
```

### Remote Connection (Phase 5)
```
ractor@local > connect 127.0.0.1:9002
Connecting to 127.0.0.1:9002
  Starting local NodeServer...
  ✓ NodeServer started on port 9100
  ✓ Connected to 127.0.0.1:9002
  Discovering introspection actor...
  ✓ pong from node_b
✓ Connected to node at 127.0.0.1:9002

ractor@local > nodes
Connected Nodes (1):
  • 127.0.0.1:9002

ractor@local > use 127.0.0.1:9002
✓ Switched to node 127.0.0.1:9002

ractor@127.0.0.1:9002 > registry
Registered Actors on 127.0.0.1:9002 (2):
  ping_pong → 1.0
  introspection → 1.1

ractor@127.0.0.1:9002 > pg members ping_pong
+----------+-------------+-------+
| Actor ID | Name        | Local |
+----------+-------------+-------+
| 1.0      | ping_pong   | yes   |
| 2.0      | ping_pong   | no    |
+----------+-------------+-------+
```

### Cluster Topology (Phase 6)
```
ractor@local > connect 127.0.0.1:9002
Connecting to 127.0.0.1:9002
  Starting local NodeServer...
  ✓ NodeServer started on port 9100
  ✓ Connected to 127.0.0.1:9002
  Discovering introspection actor...
  ✓ pong from node_b
✓ Connected to node at 127.0.0.1:9002
  Discovering cluster topology...
  ✓ Discovered 2 nodes and 2 process groups

ractor@local > cluster
Fetching cluster topology...

Cluster Nodes (2):

+---------+-----------+--------+-------+
| Node ID | Node Name | Actors | Local |
+---------+-----------+--------+-------+
| 1       | node_b    | 2      | yes   |
| 2       | node_a    | 0      | no    |
+---------+-----------+--------+-------+

ractor@local > cluster groups
Fetching cluster topology...

Process Groups (2):

Group: ping_pong
+----------+-------------+--------------+
| Actor ID | Name        | Node         |
+----------+-------------+--------------+
| 1.0      | ping_pong   | node_b (1)   |
| 2.0      | ping_pong   | node_a (2)   |
+----------+-------------+--------------+

Group: ractor_shell_introspection
+----------+----------------+--------------+
| Actor ID | Name           | Node         |
+----------+----------------+--------------+
| 1.1      | introspection  | node_b (1)   |
| 2.1      | introspection  | node_a (2)   |
+----------+----------------+--------------+
```

### Actor Monitoring (Phase 3)
```
ractor@local > registry
Registered Actors (5):
  demo_actor_1 → 0.1
  demo_actor_2 → 0.2
  demo_actor_3 → 0.3
  panicky_actor → 0.4
  shell_monitor → 0.0

ractor@local > monitor demo_actor_1
✓ Monitoring demo_actor_1 (0.1)

ractor@local > monitors
Monitored Actors:
  • demo_actor_1

ractor@local > stop demo_actor_1
✓ Sent stop signal to 'demo_actor_1'

# When fully integrated, you'll see:
# [14:24:12.456] ▼ STOPPED demo_actor_1 (0.1) - Stopped by shell
```

## Architecture

```
┌─────────────────────────────────┐
│         ractor-shell            │
├─────────────────────────────────┤
│  ┌──────────┐  ┌──────────────┐ │
│  │ REPL     │→ │ ShellState   │ │
│  │ Loop     │  │ Coordinator  │ │
│  └──────────┘  └──────────────┘ │
│       ↓               ↓          │
│  ┌──────────┐  ┌──────────────┐ │
│  │ rustyline│  │ Command      │ │
│  │ readline │  │ Dispatcher   │ │
│  └──────────┘  └──────────────┘ │
└─────────────────────────────────┘
         ↓
┌─────────────────────────────────┐
│      Ractor Runtime             │
│  ┌──────────┐  ┌──────────────┐ │
│  │ Registry │  │ Process      │ │
│  │          │  │ Groups       │ │
│  └──────────┘  └──────────────┘ │
└─────────────────────────────────┘
```

## Roadmap

See [REPL_PLANNING.md](../REPL_PLANNING.md) for the full feature roadmap.

### Phase 1: Local Foundation ✅
- Basic REPL with rustyline
- Local actor introspection
- Process group queries
- Actor control (stop)

### Phase 2: Introspection & Display ✅
- `stats` command showing system statistics
- `tree` command with visual process group hierarchy
- Actor count breakdowns by status
- Cluster-aware statistics
- Works both locally and remotely
- See [TESTING_PHASE2.md](TESTING_PHASE2.md) for testing guide
- **Note**: True supervision trees require ractor core API additions

### Phase 3: Linking & Monitoring ✅
- `monitor <actor>` command to start tracking actor lifecycle events
- `unmonitor <actor>` command to stop monitoring
- `monitors` command to list currently monitored actors
- MonitorActor infrastructure for event tracking
- Colored, timestamped event display formatting
- Event history with configurable size
- See [MONITORING.md](MONITORING.md) for complete guide
- See [TESTING_PHASE3.md](TESTING_PHASE3.md) for testing guide
- **Note**: Real-time event display pending deeper ractor supervision integration

### Phase 5: Remote Connection ✅
- `connect` to remote ractor_cluster nodes
- Multi-node context switching with `use` command
- Remote command execution via IntrospectionActor
- RPC-based introspection protocol
- See [TESTING_PHASE5.md](TESTING_PHASE5.md) for testing guide

### Phase 6: Mesh Topology Awareness ✅
- Automatic cluster topology discovery on connect
- `cluster` command with multiple subcommands
- Cluster-wide node visibility
- Cross-cluster process group queries
- View all actors across the mesh
- Topology caching for performance
- See [TESTING_PHASE6.md](TESTING_PHASE6.md) for testing guide

### Phase 4: File & Message Input ✅
- Enhanced `send` and `call` commands with JSON parsing
- `send-file` command for message construction from files
- `load` command for script execution
- **DynamicMessage interface** - Actors can opt-in to receive JSON from the shell
- **Actual message sending** - Works for actors using `DynamicMessage`
- Auto-detection of dynamic message support
- Educational explanations for non-dynamic actors
- See [TESTING_PHASE4.md](TESTING_PHASE4.md) for testing guide
- See [DYNAMIC_MESSAGES.md](DYNAMIC_MESSAGES.md) for implementation guide

### Phase 7: UX Polish ✅
- **Tab completion** - Context-aware completion for all commands and arguments
- **Command aliases** - Short forms like `a`, `r`, `i`, `s`, `c`, etc.
- **Smart hints** - Inline hints showing what aliases expand to
- **Persistent history** - Command history with arrow key navigation
- Better error messages and visual feedback
- See [UX_FEATURES.md](UX_FEATURES.md) for complete guide
- See [TESTING_PHASE7.md](TESTING_PHASE7.md) for testing guide

## Contributing

This is an experimental project. See [REPL_PLANNING.md](../REPL_PLANNING.md) for design decisions and open questions.

## License

Same as ractor parent project (MIT).

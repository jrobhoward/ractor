# Actor Monitoring Guide

This guide explains how to use the actor monitoring features in the ractor shell, which provide Erlang-style process monitoring for tracking actor lifecycle events.

## Overview

Actor monitoring allows you to observe lifecycle events for specific actors in real-time. When monitoring an actor, you'll see events for:

- **Actor Started** - When an actor begins execution
- **Actor Stopped** - When an actor terminates normally
- **Actor Panicked** - When an actor crashes due to an error
- **Actor Killed** - When an actor is forcibly terminated

## Quick Start

```bash
# Start the monitoring demo
cargo run --example monitoring_demo

# In the shell:
ractor@local > registry                # See all actors
ractor@local > monitor demo_actor_1    # Start monitoring
ractor@local > monitors                # List monitored actors
ractor@local > stop demo_actor_1       # Trigger a stop event
ractor@local > unmonitor demo_actor_2  # Stop monitoring
```

## Commands

### monitor <actor>

Start monitoring an actor's lifecycle events.

**Usage:**
```
monitor <actor_name>
```

**Example:**
```
ractor@local > monitor demo_actor_1
✓ Monitoring demo_actor_1 (0.1)
```

**Notes:**
- The actor must be registered (have a name) to be monitored
- You can monitor multiple actors simultaneously
- Events are displayed in real-time as they occur

### unmonitor <actor>

Stop monitoring an actor.

**Usage:**
```
unmonitor <actor_name>
```

**Example:**
```
ractor@local > unmonitor demo_actor_1
✓ Stopped monitoring demo_actor_1
```

### monitors

List all currently monitored actors.

**Usage:**
```
monitors
```

**Example:**
```
ractor@local > monitors
Monitored Actors:
  • demo_actor_1
  • demo_actor_2
  • demo_actor_3
```

When no actors are being monitored:
```
ractor@local > monitors
No actors currently being monitored
```

## Event Display Format

When a monitored actor experiences a lifecycle event, it's displayed with:

- **Timestamp** - Precise time of the event (HH:MM:SS.mmm)
- **Symbol** - Visual indicator of event type
- **Event Type** - START, STOPPED, PANICKED, or KILLED
- **Actor Name** - The monitored actor's name
- **Actor ID** - The actor's unique identifier
- **Additional Info** - Reason for stop/panic, error messages, etc.

### Event Examples

**Actor Started:**
```
[14:23:45.123] ▲ STARTED demo_actor_1 (0.1)
```

**Actor Stopped:**
```
[14:24:12.456] ▼ STOPPED demo_actor_1 (0.1) - Normal shutdown
```

**Actor Panicked:**
```
[14:25:03.789] ✗ PANICKED demo_actor_2 (0.2) - Division by zero
```

**Actor Killed:**
```
[14:26:30.012] ⊗ KILLED demo_actor_3 (0.3)
```

## Tab Completion

The monitor and unmonitor commands support tab completion for actor names:

```
ractor@local > monitor dem<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3

ractor@local > monitor demo_actor_<TAB>
→ demo_actor_1
```

## Use Cases

### 1. Debugging Actor Lifecycle Issues

Monitor actors to understand when and why they're stopping or crashing:

```
ractor@local > monitor problematic_actor
ractor@local > send problematic_actor {"command": "trigger_issue"}
[14:30:15.234] ✗ PANICKED problematic_actor (0.5) - Invalid state transition
```

### 2. Tracking System Health

Monitor critical actors to ensure they're running:

```
ractor@local > monitor api_handler
ractor@local > monitor database_pool
ractor@local > monitor message_queue
ractor@local > monitors
Monitored Actors:
  • api_handler
  • database_pool
  • message_queue
```

### 3. Development and Testing

Watch actor behavior during development:

```
ractor@local > monitor test_actor
ractor@local > call test_actor {"command": "run_test"}
[14:35:20.567] ▲ STARTED test_actor (0.7)
[14:35:22.890] ▼ STOPPED test_actor (0.7) - Test completed
```

### 4. Cluster Monitoring

Monitor actors across distributed nodes:

```
ractor@local > connect 127.0.0.1:9002
ractor@local > use 127.0.0.1:9002
ractor@127.0.0.1:9002 > registry
ractor@127.0.0.1:9002 > monitor remote_worker
```

## Architecture

### Monitor System Components

```
┌──────────────────────────────────────────────┐
│          MonitorActor (shell_monitor)        │
│  - Tracks which actors are monitored         │
│  - Receives SupervisionEvent notifications   │
│  - Formats and displays events               │
│  - Maintains event history                   │
└──────────────────────────────────────────────┘
                     ↓
         SupervisionEvent stream
                     ↓
┌──────────────────────────────────────────────┐
│              Monitored Actors                 │
│  - demo_actor_1                               │
│  - demo_actor_2                               │
│  - ...                                        │
└──────────────────────────────────────────────┘
```

### Event Flow

1. User runs `monitor <actor>`
2. MonitorActor looks up actor in registry
3. Actor ID is stored in monitored set
4. MonitorActor subscribes to SupervisionEvent stream
5. When actor events occur, MonitorActor receives them
6. Events are filtered (only monitored actors shown)
7. Events are formatted with colors and timestamps
8. Events are displayed to user and stored in history

## Event History

The monitoring system maintains a history of recent events (default: last 100 events).

**Current Limitations:**
- Event history is in-memory only (lost on shell restart)
- No way to query historical events yet
- No event filtering or search

**Future Enhancements:**
- `history <actor>` - Show event history for an actor
- `history --filter PANICKED` - Filter events by type
- Event persistence to file
- Event export (JSON, CSV)

## Configuration

The monitoring system can be customized through the `MonitorState` struct:

```rust
pub struct MonitorState {
    monitored: HashMap<String, ractor::ActorId>,
    event_history: Vec<MonitorEvent>,
    max_history: usize,  // Default: 100
}
```

To change the history size, modify `max_history` in the `Default` implementation.

## Comparison to Erlang

The ractor shell monitoring is inspired by Erlang's process monitoring:

| Erlang              | Ractor Shell         |
|---------------------|---------------------|
| `erlang:monitor/2`  | `monitor <actor>`   |
| `erlang:demonitor/1`| `unmonitor <actor>` |
| Monitor messages    | Event display       |
| `{'DOWN', ...}`     | MonitorEvent        |

**Key Differences:**
- Erlang sends monitor messages to the monitoring process
- Ractor shell displays events to the interactive terminal
- Erlang monitors are process-to-process
- Ractor shell uses a centralized MonitorActor

## Troubleshooting

### "Monitor system not available"

The monitor system failed to start. Check:
- MonitorActor spawned successfully during shell initialization
- No actor ID conflicts (unlikely)

### "Actor not found in registry"

The actor you're trying to monitor doesn't exist or isn't registered:
- Run `registry` to see all registered actors
- Check the actor name spelling
- Ensure the actor has been spawned with a name

### Events not appearing

The monitoring system tracks actors but you're not seeing events:
- This is a known limitation in the current implementation
- The event notification system needs to be wired up to ractor's supervision properly
- For now, the infrastructure is in place for future enhancement

### Cannot monitor unnamed actors

Currently, only named/registered actors can be monitored:
- Actors must be spawned with `Some(name)` to be monitorable
- Use `registry` to see monitorable actors
- Unnamed actors don't appear in the registry

## Examples

See `examples/monitoring_demo.rs` for a complete working example demonstrating:
- Starting and stopping actors
- Monitoring lifecycle events
- Handling panics and errors
- Managing multiple monitored actors

## Related Commands

- `registry` - List all registered (monitorable) actors
- `actors` - List all actors (including unnamed)
- `info <actor>` - Show detailed actor information
- `stop <actor>` - Stop an actor (triggers stop event)

## Future Enhancements

Planned improvements for the monitoring system:

1. **Enhanced Event Display**
   - Event filtering by type
   - Event search and query
   - Tail mode (follow events in real-time)

2. **Event History**
   - Persistent event log
   - `history` command to query past events
   - Event export (JSON, CSV, formatted text)

3. **Advanced Monitoring**
   - Actor linking (bidirectional monitoring)
   - Process group monitoring (monitor all members)
   - Custom event handlers

4. **Distributed Monitoring**
   - Monitor remote actors
   - Aggregate events across cluster
   - Remote event streaming

5. **Performance Metrics**
   - Message rates
   - Processing times
   - Resource usage

## See Also

- [TESTING_PHASE3.md](TESTING_PHASE3.md) - Testing guide for monitoring
- [src/monitor.rs](src/monitor.rs) - Monitor implementation
- [examples/monitoring_demo.rs](examples/monitoring_demo.rs) - Working example

# Phase 3 Testing Guide - Actor Monitoring

This guide shows how to test the actor monitoring features in the ractor shell.

## What's New in Phase 3

Phase 3 adds Erlang-style actor monitoring:
- **monitor** - Start monitoring an actor's lifecycle events
- **unmonitor** - Stop monitoring an actor
- **monitors** - List all monitored actors
- **Event Display** - Real-time colored event notifications

## Prerequisites

- Build the project: `cargo build --examples`
- Completed Phases 1, 2, 4, 5, 6, and 7

## Test Procedure

### Start the Monitoring Demo

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments/ractor_shell
cargo run --example monitoring_demo
```

You should see:

```
🚀 Ractor Shell - Monitoring Demo

This demo shows actor lifecycle monitoring.
Actors will start and you can monitor their lifecycle events.

✓ Monitor system started
Spawning demo actors...
✓ demo_actor_1 started
✓ demo_actor_2 started
✓ demo_actor_3 started

Available commands:
  monitor <actor>     - Start monitoring an actor
  unmonitor <actor>   - Stop monitoring an actor
  monitors            - List monitored actors
  stop <actor>        - Stop an actor (see stop event)
  registry            - Show all actors
  help                - Show all commands

Try:
  monitor demo_actor_1
  monitors
  stop demo_actor_1
```

## Testing Basic Monitoring

### 1. List Available Actors

First, see which actors are available to monitor:

```
ractor@local > registry
Registered Actors (5):
  demo_actor_3 → 0.3
  shell_monitor → 0.0
  demo_actor_1 → 0.1
  demo_actor_2 → 0.2
  panicky_actor → 0.4
```

Note the `shell_monitor` actor - this is the monitoring system itself!

### 2. Start Monitoring an Actor

Use tab completion to start monitoring:

```
ractor@local > monitor dem<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3

ractor@local > monitor demo_actor_1<ENTER>
✓ Monitoring demo_actor_1 (0.1)
```

### 3. List Monitored Actors

Check which actors are being monitored:

```
ractor@local > monitors
Monitored Actors:
  • demo_actor_1
```

### 4. Monitor Multiple Actors

You can monitor several actors at once:

```
ractor@local > monitor demo_actor_2
✓ Monitoring demo_actor_2 (0.2)

ractor@local > monitor demo_actor_3
✓ Monitoring demo_actor_3 (0.3)

ractor@local > monitors
Monitored Actors:
  • demo_actor_1
  • demo_actor_2
  • demo_actor_3
```

### 5. Stop Monitoring

Remove an actor from the monitored list:

```
ractor@local > unmonitor demo_actor_2
✓ Stopped monitoring demo_actor_2

ractor@local > monitors
Monitored Actors:
  • demo_actor_1
  • demo_actor_3
```

## Testing Event Display

### Current Limitation

In the current implementation, the event display system is in place but not yet fully wired to ractor's supervision system. You'll see confirmation messages when starting/stopping monitoring, but lifecycle events are not yet displayed in real-time.

**What Works:**
- ✅ monitor/unmonitor/monitors commands
- ✅ Actor tracking
- ✅ Event formatting code
- ✅ MonitorActor infrastructure

**What's Pending:**
- ⏳ Real-time event notifications
- ⏳ Integration with ractor's SupervisionEvent system
- ⏳ Event history queries

### Future Event Display

When fully implemented, you'll see events like:

```
ractor@local > monitor demo_actor_1
✓ Monitoring demo_actor_1 (0.1)

ractor@local > stop demo_actor_1
[14:24:12.456] ▼ STOPPED demo_actor_1 (0.1) - Stopped by shell
✓ Sent stop signal to 'demo_actor_1'
```

## Testing Tab Completion

### 1. Command Completion

Monitor commands support tab completion:

```
ractor@local > mon<TAB>
→ monitor

ractor@local > unmon<TAB>
→ unmonitor

ractor@local > moni<TAB>
→ monitor | monitors
```

### 2. Actor Name Completion

Actor names auto-complete for monitor/unmonitor:

```
ractor@local > monitor <TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3 | panicky_actor

ractor@local > monitor demo_<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3

ractor@local > unmonitor demo_actor_<TAB>
→ demo_actor_1
```

## Testing Help System

### 1. General Help

The help command shows monitoring commands:

```
ractor@local > help

...

  Actor Monitoring:
  monitor <actor>    Start monitoring actor events
  unmonitor <actor>  Stop monitoring an actor
  monitors          List monitored actors

...
```

### 2. Command-Specific Help

Get detailed help for each command:

```
ractor@local > help monitor
monitor <actor>
  Start monitoring an actor's lifecycle events

Usage:
  monitor <actor_name>

Example:
  monitor demo_actor_1

Note:
  - Shows start, stop, panic, and kill events
  - Events are displayed in real-time


ractor@local > help unmonitor
unmonitor <actor>
  Stop monitoring an actor

Usage:
  unmonitor <actor_name>

Example:
  unmonitor demo_actor_1


ractor@local > help monitors
monitors
  List all currently monitored actors

Usage:
  monitors
```

## Testing Error Handling

### 1. Monitor Non-Existent Actor

Try to monitor an actor that doesn't exist:

```
ractor@local > monitor nonexistent_actor
✗ Actor 'nonexistent_actor' not found in registry
```

### 2. Unmonitor Non-Monitored Actor

Try to unmonitor an actor that isn't being monitored:

```
ractor@local > unmonitor demo_actor_1
✗ Not monitoring 'demo_actor_1'
```

### 3. List When No Monitors

Check the monitors list when nothing is being monitored:

```
ractor@local > monitors
No actors currently being monitored
```

## Testing Integration with Other Commands

### 1. Combine with Registry

Use registry to see which actors can be monitored:

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
```

### 2. Combine with Info

Get details about a monitored actor:

```
ractor@local > monitor demo_actor_1
✓ Monitoring demo_actor_1 (0.1)

ractor@local > info demo_actor_1
Actor: demo_actor_1
  ID:     0.1
  Status: Running
  Name:   demo_actor_1
```

### 3. Combine with Stop

Stop a monitored actor:

```
ractor@local > monitor demo_actor_1
✓ Monitoring demo_actor_1 (0.1)

ractor@local > stop demo_actor_1
✓ Sent stop signal to 'demo_actor_1'
```

## Testing Script Loading

Create a script to automate monitoring setup:

### Create `scripts/monitor_all.txt`

```bash
cat > scripts/monitor_all.txt << 'EOF'
# Monitor all demo actors
registry
monitor demo_actor_1
monitor demo_actor_2
monitor demo_actor_3
monitors
EOF
```

### Run the Script

```
ractor@local > load scripts/monitor_all.txt
load scripts/monitor_all.txt

✓ Loaded script from scripts/monitor_all.txt

• Executing 5 commands...

▸ [1] registry
Registered Actors (5):
  demo_actor_1 → 0.1
  demo_actor_2 → 0.2
  demo_actor_3 → 0.3
  panicky_actor → 0.4
  shell_monitor → 0.0

▸ [2] monitor demo_actor_1
✓ Monitoring demo_actor_1 (0.1)

▸ [3] monitor demo_actor_2
✓ Monitoring demo_actor_2 (0.2)

▸ [4] monitor demo_actor_3
✓ Monitoring demo_actor_3 (0.3)

▸ [5] monitors
Monitored Actors:
  • demo_actor_1
  • demo_actor_2
  • demo_actor_3

✓ Script execution complete
```

## Testing with Other Examples

### 1. Dynamic Actor Example

The monitoring works with other examples too:

```bash
cargo run --example dynamic_actor

ractor@local > registry
ractor@local > monitor dynamic_actor
ractor@local > send dynamic_actor {"command": "increment"}
ractor@local > monitors
```

### 2. Regular Demo Example

```bash
cargo run --example demo

ractor@local > registry
ractor@local > monitor demo_actor_1
ractor@local > monitors
```

## Verification Checklist

After testing, verify:

- ✅ Monitor system starts successfully (shell_monitor actor)
- ✅ Can monitor registered actors
- ✅ Can unmonitor actors
- ✅ monitors command shows tracked actors
- ✅ Tab completion works for monitor/unmonitor
- ✅ Help text available for all monitoring commands
- ✅ Error messages for invalid actors
- ✅ Integration with registry/info/stop commands
- ✅ Script loading works with monitoring commands

## Known Limitations

1. **Event Display Not Fully Implemented**
   - Infrastructure is in place
   - Real-time events not yet displayed
   - Requires deeper ractor supervision integration

2. **No Event History Yet**
   - Events are tracked internally
   - No command to query history
   - Future: `history <actor>` command

3. **Only Named Actors**
   - Can only monitor registered actors
   - Unnamed actors not supported
   - Use `registry` to see monitorable actors

4. **No Distributed Monitoring Yet**
   - Cannot monitor remote actors
   - Future: cluster-wide monitoring
   - Commands work but need remote integration

## Next Steps

After verifying Phase 3 monitoring:

1. **Explore Event System**
   - Review `src/monitor.rs` implementation
   - Understand MonitorEvent structure
   - See how events are formatted

2. **Try Monitoring in Real Scenarios**
   - Monitor actors during development
   - Track lifecycle of critical actors
   - Use with cluster topology

3. **Provide Feedback**
   - Report issues or suggestions
   - Request additional monitoring features
   - Share use cases

## Future Enhancements

Planned improvements for monitoring:

1. **Real-Time Events** - Wire up SupervisionEvent properly
2. **Event History** - Query past events with `history` command
3. **Event Filtering** - Filter by type (START, STOP, PANIC)
4. **Remote Monitoring** - Monitor actors on other nodes
5. **Process Group Monitoring** - Monitor all members of a group
6. **Event Export** - Save events to file (JSON, CSV)

## Phase 3 Complete!

You've successfully tested:
- ✓ Monitor command (start tracking actors)
- ✓ Unmonitor command (stop tracking actors)
- ✓ Monitors command (list tracked actors)
- ✓ Tab completion for monitoring
- ✓ Help system integration
- ✓ Error handling
- ✓ Script automation

The monitoring infrastructure is in place and ready for future enhancement!

## See Also

- [MONITORING.md](MONITORING.md) - Complete monitoring guide
- [src/monitor.rs](src/monitor.rs) - Monitor implementation
- [examples/monitoring_demo.rs](examples/monitoring_demo.rs) - Working example

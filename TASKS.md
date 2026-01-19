# ractor_shell Tasks

**Branch**: phase2/introspection
**Goal**: Rust-ractor counterpart for Erlang/OTP's shell & observer
**Status**: Core features complete. Enhancements in progress.

**Recent Completion**: Actor metrics added to ractor core (`get_message_count()`, `get_handle_time_ns()`) with Msg/s column in `top` TUI.

**Erlang/OTP Comparison**: See [Feature Comparison](#erlang-otp-feature-comparison) section.

---

## Priority 1: Supervision Tree Visualization ✅ COMPLETE

**Estimated Time**: 3-4 hours
**Value**: HIGH - True Erlang Observer parity
**Core Changes**: None needed - `get_children()` and `try_get_supervisor()` are already public on `ActorCell`!

- [x] **Add `supervtree` command for supervision tree display**
  - Use `ActorCell::get_children()` to walk the tree
  - Use `ActorCell::try_get_supervisor()` to find roots
  - ASCII tree visualization like `pstree`
  - Show: actor name, ID, status, child count
  - Works both locally and remotely (via RPC)
  - Example output:
    ```
    supervisor (0.1) [Running]
    ├── worker_1 (0.2) [Running]
    ├── worker_2 (0.3) [Running]
    └── sub_supervisor (0.4) [Running]
        ├── sub_worker_1 (0.5) [Running]
        └── sub_worker_2 (0.6) [Running]
    ```

- [ ] **Add supervision tree panel to `top` TUI**
  - New panel showing tree structure
  - Tab key to switch between actors list and tree view
  - Highlight selected actor's position in tree

- [x] **Add `parent <actor>` command**
  - Show the supervisor of a given actor
  - Erlang equivalent: `process_info(Pid, links)`
  - Works both locally and remotely (via RPC)

- [x] **Update `info <actor>` to show supervision info**
  - Add "Supervisor: <name>" field
  - Add "Children: N" field with names listed

---

## Priority 2: Raft Debugging & Observability

**Estimated Time**: 3-4 hours
**Value**: HIGH - Essential for observing and debugging Raft cluster behavior
**Core Changes**: None

- [x] **DynamicMessage investigation** ✅
  - DynamicMessage is still used and necessary for `send` command and flexible debugging
  - Raft already uses typed `RaftMessage` with schema registration (not DynamicMessage)
  - Two approaches coexist appropriately: DynamicMessage for flexibility, typed schemas for performance
  - No changes needed - current architecture is sound

- [x] **Enable Raft debug logging in shell tracing** ✅ COMPLETE
  - Implemented option 2: Shell tracing now matches against any field value
  - Added `matches_any_field()` method to `TraceFilter`
  - Updated `layer.rs` to check field values in addition to actor_name/actor_id/target
  - Now `trace node_*` will capture Raft events with `node = "node_a"` etc.
  - Example: `trace node_a` or `trace node_*` will capture Raft logs
  - File: `ractor_shell/src/tracing/filter.rs:220-241` (matches_any_field)
  - File: `ractor_shell/src/tracing/layer.rs:418-421` (filtering logic)

- [x] **Add command to trigger fresh leader election** ✅ COMPLETE
  - Added `StepDown(RpcReplyPort<bool>)` RPC to `RaftMessage` enum
  - Use via generic call command: `call raft_node StepDown {}`
  - Returns true if leader stepped down, false if not leader
  - Works both locally and on remote nodes via typed RPC
  - Updated `test_cluster.sh` help and quick commands with StepDown
  - Updated `raft.rs` module documentation
  - File: `ractor_shell/examples/cluster_demo/raft.rs` (StepDown handler)
  - Note: No shell-specific `raft` command - uses generic `call` mechanism

- [x] **Add `raft status` command enhancements** ✅ COMPLETE
  - Enhanced `RaftStatus` struct with new fields:
    - `peer_names: Vec<String>` - names of known peers (not just count)
    - `election_generation: u64` - election timer generation
    - `ms_since_heartbeat: Option<u64>` - milliseconds since last heartbeat (followers only)
    - `votes_received: usize` - vote count (useful when Candidate)
  - Added `last_heartbeat_received: Option<Instant>` to `RaftState`
  - Example: `call raft_node GetStatus {}` now shows full diagnostics

- [x] **Investigate raft_node behavior on node failure** ✅ RESOLVED - NOT A BUG
  - **Original Observation**: When node_a was killed, local actors on node_c became unreachable
  - **Root Cause**: Cookie mismatch during manual testing
    - The shell uses `secret_cookie` as its default cluster cookie
    - Manual testing was using `--cookie test` for nodes
    - Cookie mismatch causes authentication to fail silently
    - Failed auth = no process group sync = "No introspection actor found"
  - **Verified Behavior**:
    - With matching cookies, cluster works correctly
    - Local actors on surviving nodes remain fully operational after node failure
    - `call raft_node GetStatus {}` works correctly after killing other nodes
    - NodeSession cleanup logic is correct
  - **Lesson**: Always use matching cookies (`secret_cookie` is the default)
  - **Files reviewed**:
    - `ractor_cluster/src/node/node_session.rs` - cleanup logic is correct
    - `ractor_cluster/src/node.rs` - NodeServer supervision is correct

---

## Priority 3: Remote Command Support

**Estimated Time**: 4-6 hours
**Value**: HIGH - The primary use case is remote interaction, not local monitoring
**Core Changes**: None - protocol changes only

**Philosophy**: The shell's primary purpose is interacting with remote nodes. All commands that make sense remotely should work in remote context.

### Currently Remote-Capable:
- [x] `actors` - Lists actors on remote node
- [x] `registry` - Lists registered actors on remote node
- [x] `info` - Shows actor info from remote node
- [x] `schema` - Shows message schema from remote node
- [x] `send` / `call` - Sends messages to remote actors
- [x] `stats` - Shows stats from remote node
- [x] `supervtree` - Shows supervision tree from remote node
- [x] `parent` - Shows parent from remote node
- [x] `pg members` - Lists process group members from remote node
- [x] `pg list` - Lists all process groups on remote node
- [x] `stop` - Stops an actor on remote node (with proper error reporting)
- [x] `tree` - Shows process group tree on remote node
- [x] `monitor` / `unmonitor` / `monitors` - Remote monitoring with status-based polling

### Need Remote Support:

- [x] **Add remote `stop` command** ✅ COMPLETE
  - Added `StopActor(String, RpcReplyPort<bool>)` to protocol
  - IntrospectionActor calls `registry::where_is()` then `cell.stop()`
  - Returns true if actor found and stopped, false if not found
  - Shell reports proper error when actor not found

- [x] **Add remote `pg list` command** ✅ COMPLETE
  - Added `ListProcessGroups(RpcReplyPort<Vec<String>>)` to protocol
  - IntrospectionActor calls `pg::which_groups()`
  - Shell displays list of all process groups on remote node

- [x] **Add remote `tree` command** (process group tree) ✅ COMPLETE
  - Added `GetProcessGroupTree(RpcReplyPort<HashMap<String, Vec<ActorInfo>>>)` to protocol
  - Returns all process groups with their members in one RPC call
  - Local tree now uses `pg::which_groups()` for dynamic discovery (no more hardcoded list)

- [x] **Add remote `monitor` / `unmonitor` / `monitors` commands** ✅ COMPLETE
  - Added `StartMonitoring(String, RpcReplyPort<bool>)` to protocol
  - Added `StopMonitoring(String, RpcReplyPort<bool>)` to protocol
  - Added `GetMonitoredActors(RpcReplyPort<Vec<String>>)` to protocol
  - Added `PollMonitorEvents(RpcReplyPort<MonitorEventBatch>)` to protocol
  - IntrospectionActor tracks monitored actors and their status changes
  - Added `monitor events` command to poll and display events from remote node
  - Local events displayed automatically, remote events via polling

### Remote `top` Support:

- [x] **Add `GetActorMetrics` protocol message** ✅ COMPLETE
  - Added `RemoteActorMetrics` struct in `protocol.rs` (serializable version of ActorMetrics)
  - Added `GetActorMetrics(RpcReplyPort<Vec<RemoteActorMetrics>>)` to ShellProtocolMessage
  - IntrospectionActor collects metrics from registry and process groups
  - Returns: actor ID, name, status, process groups, uptime_ms, message_count

- [x] **Add `GetSupervisionTree` protocol message**
  - Returns tree structure for remote visualization
  - Enables remote `supervtree` command
  - Added `SupervisionTreeNode` serializable structure

- [x] **Add `GetActorParent` protocol message**
  - Returns parent/supervisor info for an actor
  - Enables remote `parent` command

- [x] **Update `top` to work with remote context** ✅ COMPLETE
  - `top` command now checks `current_node` and uses remote RPC if connected
  - TUI header shows node name when in remote mode (e.g., "ractor top - 127.0.0.1:9001 (remote)")
  - Error messages displayed in header if RPC fails (timeout, connection lost)
  - Added `App::run_remote()` and `App::new_remote()` methods in `tui/app.rs`

---

## Priority 4: Test Coverage Gaps

**Estimated Time**: 4-6 hours
**Value**: MEDIUM - Important for reliability
**Core Changes**: None

- [x] **Add tests for `introspection.rs`** ✅ COMPLETE
  - Added 34 tests covering all key functions
  - TraceSubscription, pattern matching, extract_node_id
  - send/call_dynamic_message_to_actor, call_typed_rpc_on_actor
  - build_cluster_topology, build_supervision_tree_roots
  - IntrospectionActor integration tests (spawn, ping, list, get info)

- [ ] **Add tests for `tui/app.rs`**
  - 332 lines with no tests
  - State machine logic: sorting, filtering, navigation
  - ~1-2 hours

- [ ] **Expand `monitor/monitor_tests.rs`**
  - Currently only 1 test for 328-line module
  - Test lifecycle event handling
  - ~1 hour

- [ ] **Add tests for `error.rs`**
  - Test error creation helpers
  - ~30 minutes

---

## Priority 5: Simple Watch Mode

**Estimated Time**: 2-3 hours
**Value**: MEDIUM - Lightweight alternative to full TUI
**Core Changes**: None

- [ ] **Add `watch` command with auto-refresh**
  - `watch actors` - refresh actor list every N seconds
  - `watch stats` - continuous system stats
  - `watch pg <group>` - monitor process group membership
  - `watch tree` - monitor supervision tree changes
  - Configurable refresh interval (default 2s)
  - Press 'q' or Ctrl+C to exit watch mode

---

## Priority 6: Network/Cluster Diagnostics

**Estimated Time**: 3-4 hours
**Value**: MEDIUM - Important for distributed debugging
**Core Changes**: None

- [ ] **Enhance `nodes` command with connection stats**
  - Show connection duration, last activity, connection state

- [x] **Add `ping <node>` command** ✅ COMPLETE
  - Explicit latency measurement to remote node
  - Erlang equivalent: `net_adm:ping/1`
  - Auto-connects if not already connected
  - Uses ShellProtocolMessage::Ping to measure round-trip time

- [ ] **Add `netstat` command for cluster connections**
  - Show bytes sent/received, pending message counts
  - Erlang equivalent: `inet:getstat/1`

- [ ] **Handle stale cluster connections gracefully**
  - **Problem**: Shell stores `ActorRef<ShellProtocolMessage>` to remote introspection actors.
    When the underlying ractor_cluster connection drops (network hiccup, node restart),
    the ActorRef becomes stale but the shell has no notification. Subsequent sends fail with `SendErr`.
  - **Current Workaround**: Added `reconnect <node>` command to manually refresh connections.
  - **Potential Solutions**:
    1. **Auto-reconnect on SendErr**: Detect the error, reconnect, and retry the operation automatically
    2. **Periodic health checks**: Background task pings connected nodes to detect staleness early
    3. **Connection event monitoring**: Subscribe to ractor_cluster connection events (may need core support)
    4. **Retry wrapper**: Wrap all remote operations with retry-on-reconnect logic
  - **Investigation needed**: Check if ractor_cluster provides connection status callbacks or events
  - **Files**: `lib.rs` (cmd_reconnect, connected_nodes), `error.rs` (MessagingError)

---

## Priority 7: Code Cleanup

**Estimated Time**: 1.5 hours
**Value**: LOW - Consistency with SKILLS.md
**Core Changes**: None

- [ ] **Move `tui/metrics.rs` inline tests to separate file**
  - Currently uses inline `#[cfg(test)] mod tests` block
  - Should be `tui/metrics/metrics_tests.rs`

- [ ] **Extract magic numbers in `completer_custom.rs`**
  - Lines 73, 78, 87, 256, 266, 282 have hardcoded fuzzy matching scores
  - Extract to named constants

---

## Priority 8: Enhanced Actor Inspection

**Estimated Time**: 2-3 hours
**Value**: LOW-MEDIUM
**Core Changes**: None

- [ ] **Add `whereis <name>` command (Erlang-style)**
  - Quick lookup of registered name to actor ID
  - Erlang equivalent: `whereis/1`

- [ ] **Add `links <actor>` command** (OPTIONAL - mostly covered by `supervtree` and `parent`)
  - Show supervisor and children for an actor in one view
  - Combines functionality from `parent` and `info` commands
  - Uses already-public `get_children()` and `try_get_supervisor()`

- [ ] **Clarify `tree` vs `supervtree` naming**
  - `tree` shows process groups
  - `supervtree` shows supervision hierarchy
  - Update documentation to explain difference

---

## Priority 9: Shell UX Improvements

**Estimated Time**: 2-3 hours
**Value**: LOW
**Core Changes**: None

- [ ] **Add `clear` command** - Clear terminal screen
- [ ] **Add `alias` command** - User-defined aliases, persist in config
- [ ] **Add `set` command** - Runtime configuration changes

---

## Priority 10: Schema-Based Generic RPC Dispatch ✅ COMPLETE

**Estimated Time**: Unknown - Research required
**Value**: LOW (workarounds exist)
**Difficulty**: VERY HIGH
**Core Changes**: Potentially significant

**Problem**: Remote typed RPC to cluster actors requires compile-time knowledge of message types. When actors are in example code (not the library), the introspection layer can't directly call typed RPCs.

**Status**: ✅ **Full dispatcher implemented!** The `#[ractor_shell]` macro now generates typed RPC dispatchers for all RPCs, including those with arguments.

**What Works**:
- [x] Macro generates dispatcher module for `#[ractor_shell]` enums
- [x] Dispatcher handles RPCs with only `RpcReplyPort<T>` argument
- [x] Dispatcher handles RPCs with additional arguments (e.g., `SetValue(i32, RpcReplyPort<bool>)`)
- [x] JSON argument deserialization with clear error messages for missing/invalid fields
- [x] 5-second timeout with proper error handling
- [x] Schema registry stores and retrieves dispatchers
- [x] Introspection layer calls dispatchers automatically
- [x] Works for both local and remote actors

**Implementation Details**:
- Arguments are passed via JSON with field keys "0", "1", etc. for positional fields
- Example: `call my_actor SetValue {"0": 42}` sets value to 42
- Example: `call my_actor CompareAndSet {"0": 42, "1": 99}` for compare-and-swap
- Clear error messages when fields are missing or have wrong types

**Current Workarounds** (still valid for complex cases):
1. ✅ Actors implement DynamicMessage wrapper (works, requires per-actor code)
2. ✅ Use remote tracing to observe behavior (passive observation)
3. ✅ Local typed RPC works fine (same process)

---

## Requires Ractor Core Changes

These items need modifications to ractor core. Keep changes minimal.

### Actor Metrics API (Enables enhanced `top`) ✅ COMPLETE

**Estimated Time**: 2-3 hours for core changes + 1-2 hours shell integration
**Value**: MEDIUM-HIGH

- [ ] **Add `get_start_instant()` to ActorCell**
  - Store `Instant` when actor is created
  - Enables precise uptime calculation
  - Small change: add field to `ActorCellInner`, expose via public method

- [x] **Add `get_message_count()` to ActorCell** ✅ COMPLETE
  - Added `message_count: AtomicU64` to `ActorProperties`
  - Incremented on each message processed in actor loop
  - Public getter `ActorCell::get_message_count()` exposed
  - Works for both regular and thread-local actors
  - File: `ractor/src/actor/actor_properties.rs`
  - File: `ractor/src/actor/actor_cell.rs`
  - File: `ractor/src/actor.rs` (instrumentation)
  - File: `ractor/src/thread_local/inner.rs` (instrumentation)

- [x] **Add `get_handle_time_ns()` to ActorCell** ✅ COMPLETE
  - Added `handle_time_ns: AtomicU64` to `ActorProperties`
  - Accumulates nanoseconds spent in message handlers
  - Public getter `ActorCell::get_handle_time_ns()` exposed
  - Enables future "handler latency" metrics

- [x] **Add Msg/s column to `top` TUI** ✅ COMPLETE
  - Added `handle_time_ns` and `uptime_secs` fields to `ActorMetrics`
  - Added `msg_rate()` and `msg_rate_string()` helper methods
  - Added `MsgRate` to `SortColumn` enum (sortable with 's' key)
  - Updated `RemoteActorMetrics` protocol for remote `top`
  - Uptime cached at refresh time for stable rate display
  - File: `ractor_shell/src/tui/metrics.rs`
  - File: `ractor_shell/src/tui/app.rs`
  - File: `ractor_shell/src/tui/ui.rs`
  - File: `ractor_shell/src/protocol.rs`
  - File: `ractor_shell/src/introspection.rs`

### Message Queue Depth API

**Estimated Time**: 2-3 hours
**Value**: MEDIUM - Enables backpressure detection

- [ ] **Add `get_queue_depth()` to ActorCell**
  - Expose channel length from `ActorPortSet`
  - Requires access to internal channel stats
  - Tokio mpsc channels have `len()` method

### Link/Monitor Introspection

**Estimated Time**: 1-2 hours
**Value**: LOW-MEDIUM

- [ ] **Expose monitors list on ActorCell**
  - Currently `monitors` is `pub(crate)` in supervision tree
  - Add `get_monitors()` public method
  - Erlang equivalent: `process_info(Pid, monitors)`

---

## Erlang/OTP Feature Comparison

| Feature | ractor_shell | Erlang Equivalent | Status |
|---------|-------------|-------------------|--------|
| Process listing | `actors`, `registry` | `processes()`, `registered()` | ✅ Implemented |
| Process groups | `pg list`, `pg members` | `pg:get_members/1` | ✅ Implemented |
| Basic info | `info` (ID, name, status, supervisor, children) | `process_info/1` | ✅ Implemented |
| Stop/Kill | `stop` | `exit/2` | ✅ Implemented |
| Remote connection | `connect`, `use` | `-remsh`, `net_adm:ping/1` | ✅ Implemented |
| Cluster topology | `cluster` commands | `nodes()`, observer | ✅ Implemented |
| Monitoring | `monitor`/`unmonitor` | `erlang:monitor/2` | ✅ Implemented |
| Message sending | `send`, `call` | Direct calls | ✅ DynamicMessage + typed schemas |
| Top/Dashboard TUI | `top` | observer_cli | ✅ Phase 1 Complete |
| Tracing | `trace`, `trace-to-file` | `dbg`, trace BIFs | ✅ Complete |
| **Supervision trees** | `supervtree`, `parent` | Observer supervision view | ✅ **Complete** (local + remote) |
| Message queue depth | - | `message_queue_len` | ❌ Needs core API |
| Link inspection | `links` | `links`, `monitors` | ⏳ **Priority 8** |
| Live refresh (simple) | `watch` | - | ⏳ Priority 5 |
| Memory usage | - | `memory` | ❌ Needs core API |
| Message count/rate | `top` Msg/s column | `reductions` | ✅ **Complete** (core + shell) |

---

## PR Checklist

- [x] All tests pass: `cargo test --package ractor_shell` (234 tests)
- [x] Clippy passes: `cargo clippy --package ractor_shell -- -D clippy::all -D warnings`
- [x] Rustfmt passes: `cargo fmt --package ractor_shell -- --check`
- [x] Documentation builds: `cargo doc --package ractor_shell --no-deps`
- [x] Examples run successfully
- [ ] Git history is clean

---

## Quick Reference

```bash
# Build
cargo build -p ractor_shell

# Test
cargo test --package ractor_shell

# Lint
cargo clippy --package ractor_shell -- -D clippy::all -D warnings

# Run demo
cargo run --example demo -p ractor_shell

# Run cluster demo
./ractor_shell/scripts/test_cluster.sh
```

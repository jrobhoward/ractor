# ractor_shell Tasks

**Branch**: phase2/introspection
**Goal**: Rust-ractor counterpart for Erlang/OTP's shell & observer
**Status**: Core features complete. Enhancements in progress.

**Recent Completion**: Priority 1 (Supervision Tree Visualization) is now complete with full local and remote support! This includes `supervtree`, `parent`, and enhanced `info` commands.

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

- [ ] **Enable Raft debug logging in shell tracing**
  - Raft already has `tracing::info!` calls but they use custom field names (`node`, `term`)
  - Shell tracing expects `actor_name` or `actor_id` fields to match patterns like `raft_node`
  - Options to fix:
    1. Update Raft tracing calls to include `actor_name` field: `tracing::info!(actor_name = "raft_node", ...)`
    2. Make shell tracing match against any field value, not just `actor_name`/`actor_id`
    3. Add `trace raft` command that specifically captures Raft-related module paths
  - Current workaround: Use `trace remote <node> *` to capture all events
  - File: `ractor_shell/examples/cluster_demo/raft.rs` (tracing calls throughout)
  - File: `ractor_shell/src/tracing/layer.rs:339-408` (event filtering logic)

- [ ] **Add command to trigger fresh leader election**
  - New shell command: `raft election` or `raft stepdown`
  - Forces current leader to step down, triggering new election
  - Useful for observing election protocol in action

- [ ] **Add `raft status` command enhancements**
  - Show current term, voted_for, election generation
  - Show peer connection status (connected/disconnected)
  - Show time since last heartbeat received

- [ ] **Investigate raft_node behavior on node failure** (LIKELY RACTOR_CLUSTER BUG)
  - **Observed**: When node_a was killed, local actors on node_c became unreachable
  - **Symptoms**:
    - Node_c process still running after node_a killed
    - NodeServer still accepting connections
    - But `call raft_node GetStatus {}` returns SendErr
    - IntrospectionActor not found in registry
  - **Debug logs show**:
    - TCP session closed cleanly: `Node session node_a exited with 'tcp_session_closed'`
    - No actor stop/fail events logged
    - No panics or errors
  - **Expected**: Local actors should survive remote node failures
  - **Root cause**: Unknown - likely in ractor_cluster's NodeSession cleanup logic
  - **Files to investigate**:
    - `ractor_cluster/src/node/node_session.rs` - cleanup on disconnect
    - `ractor_cluster/src/node/mod.rs` - NodeServer supervision handling
  - **This is a ractor_cluster issue, not ractor_shell**

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

### Need Remote Support:

- [ ] **Add remote `stop` command**
  - Add `StopActor(String, RpcReplyPort<Result<(), String>>)` to protocol
  - IntrospectionActor calls `registry::where_is()` then `cell.stop()`
  - Return success/failure to caller
  - ~30 minutes

- [ ] **Add remote `pg list` command**
  - Add `ListProcessGroups(RpcReplyPort<Vec<String>>)` to protocol
  - IntrospectionActor calls `pg::which_groups()`
  - ~30 minutes

- [ ] **Add remote `tree` command** (process group tree)
  - Add `GetProcessGroupTree(RpcReplyPort<...>)` to protocol
  - ~45 minutes

- [ ] **Add remote `monitor` / `unmonitor` / `monitors` commands**
  - More complex - need to relay lifecycle events back to shell
  - May need persistent subscription mechanism
  - ~2 hours

### Remote `top` Support:

- [ ] **Add `GetActorMetrics` protocol message**
  - New message in `protocol.rs`: `GetActorMetrics(RpcReplyPort<Vec<ActorMetrics>>)`
  - IntrospectionActor collects same data as local `top`
  - Return: actor ID, name, status, process groups, first-seen time

- [x] **Add `GetSupervisionTree` protocol message**
  - Returns tree structure for remote visualization
  - Enables remote `supervtree` command
  - Added `SupervisionTreeNode` serializable structure

- [x] **Add `GetActorParent` protocol message**
  - Returns parent/supervisor info for an actor
  - Enables remote `parent` command

- [ ] **Update `top` to work with remote context**
  - When `current_node` is set, fetch metrics via RPC
  - Show node name in TUI header
  - Handle latency gracefully (show stale indicator)

---

## Priority 4: Test Coverage Gaps

**Estimated Time**: 4-6 hours
**Value**: MEDIUM - Important for reliability
**Core Changes**: None

- [ ] **Add tests for `introspection.rs`** (HIGH PRIORITY)
  - 570 lines with no tests
  - Complex functions: `send_dynamic_message_to_actor()`, `call_dynamic_message_to_actor()`, `build_cluster_topology()`
  - Multiple error paths untested
  - ~2 hours

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

- [ ] **Add `ping <node>` command**
  - Explicit latency measurement to remote node
  - Erlang equivalent: `net_adm:ping/1`

- [ ] **Add `netstat` command for cluster connections**
  - Show bytes sent/received, pending message counts
  - Erlang equivalent: `inet:getstat/1`

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

## Priority 10: Schema-Based Generic RPC Dispatch ✅ PARTIAL

**Estimated Time**: Unknown - Research required
**Value**: LOW (workarounds exist)
**Difficulty**: VERY HIGH
**Core Changes**: Potentially significant

**Problem**: Remote typed RPC to cluster actors requires compile-time knowledge of message types. When actors are in example code (not the library), the introspection layer can't directly call typed RPCs.

**Status**: ✅ **Basic dispatcher implemented!** The `#[ractor_shell]` macro now generates typed RPC dispatchers for single-argument RPCs (just `RpcReplyPort<T>`). This covers most common use cases including RaftMessage's `GetStatus`, `IsLeader`, `GetLeader`, and `GetPeers` RPCs.

**What Works**:
- [x] Macro generates dispatcher module for `#[ractor_shell]` enums
- [x] Dispatcher handles RPCs with only `RpcReplyPort<T>` argument
- [x] 5-second timeout with proper error handling
- [x] Schema registry stores and retrieves dispatchers
- [x] Introspection layer calls dispatchers automatically
- [x] Works for both local and remote actors

**Remaining Enhancement** (low priority):
- [ ] **Parse arguments from JSON for RPCs with additional fields**
  - Example: `UpdateValue(i32, RpcReplyPort<bool>)` - currently returns helpful error
  - Would require JSON → typed argument deserialization in generated dispatcher
  - Macro would need to:
    1. Parse non-RpcReplyPort fields as arguments
    2. Deserialize JSON to those types using serde
    3. Pass them to the message constructor
  - See `MACRO_DISPATCHER_PLAN.md` "Future Enhancements" section
  - ~2-3 hours estimated

**Current Workarounds** (still valid for complex cases):
1. ✅ Actors implement DynamicMessage wrapper (works, requires per-actor code)
2. ✅ Use remote tracing to observe behavior (passive observation)
3. ✅ Local typed RPC works fine (same process)

---

## Requires Ractor Core Changes

These items need modifications to ractor core. Keep changes minimal.

### Actor Metrics API (Enables enhanced `top`)

**Estimated Time**: 2-3 hours for core changes + 1-2 hours shell integration
**Value**: MEDIUM-HIGH

- [ ] **Add `get_start_instant()` to ActorCell**
  - Store `Instant` when actor is created
  - Enables precise uptime calculation
  - Small change: add field to `ActorCellInner`, expose via public method

- [ ] **Add `get_message_count()` to ActorCell** (optional)
  - Counter incremented on each message processed
  - Enables "reductions" equivalent
  - More invasive: requires changes to message handling

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
| Memory/reductions | - | `memory`, `reductions` | ❌ Needs core API |

---

## PR Checklist

- [x] All tests pass: `cargo test --package ractor_shell` (137 tests)
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

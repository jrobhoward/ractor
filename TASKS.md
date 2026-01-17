# ractor_shell Integration Tasks

**Branch**: feature/shell
**Goal**: Prepare ractor_shell for PR to upstream ractor repository
**Status**: Integration Complete, Pre-PR Cleanup In Progress (1.1 ✅, 1.2 ✅, 1.3 ✅, 1.4 ✅, 1.5 ✅, 2.1 ✅, 2.2 ✅, 2.3 ✅, 2.4 ✅, 3.1 ✅, 5.1-Phase1 ✅, 5.2 ✅)

**Erlang/OTP Comparison**: See [Feature Comparison](#erlang-otp-feature-comparison) section for gaps analysis vs Erlang shell/Observer.

---

## Priority 1: Critical (Must-Have for PR)

### 1.1 Code Quality & Standards ✅

**Estimated Time**: 2-3 hours
**Status**: Complete

- [x] **Run rustfmt on ractor_shell**
  ```bash
  cargo fmt --package ractor_shell
  cargo fmt --package ractor_shell -- --check
  ```
  - Files: `ractor_shell/src/**/*.rs`, `ractor_shell/examples/**/*.rs`
  - Ensure all code follows ractor's formatting standards

- [x] **Fix all clippy warnings**
  ```bash
  cargo clippy --package ractor_shell -- -D clippy::all -D warnings
  ```
  - Address ALL clippy warnings and errors
  - Ractor CI enforces `-D warnings` (warnings are treated as errors)
  - Fixed 10 issues: redundant closures, empty doc comment lines, unused imports/variables

- [x] **Remove dead code warnings**
  - Fix or document all `#[allow(dead_code)]` attributes
  - Remove unused helper functions
  - Remove commented-out code blocks
  - Added `#[allow(dead_code)]` to demo example's unused message variant

- [x] **Verify no ractor_experiments references**
  ```bash
  rg "ractor_experiments" ractor_shell/
  ```
  - Ensure no hardcoded paths or imports from ractor_experiments
  - Updated README.md, INTEGRATION.md, REPL_PLANNING.md
  - Removed 6 TESTING_PHASE*.md files with hardcoded paths

### 1.2 Documentation ✅

**Estimated Time**: 2-4 hours
**Status**: Complete

- [x] **Add rustdoc comments to public APIs**
  - File: `ractor_shell/src/lib.rs`
    - `ShellState` struct and methods
    - `ShellCommand` enum variants with usage docs
    - Public execute functions
  - File: `ractor_shell/src/dynamic.rs` - already well documented
  - File: `ractor_shell/src/introspection.rs`
    - `IntrospectionActor` and `IntrospectionState`
  - File: `ractor_shell/src/protocol.rs`
    - `ActorInfo`, `ActorLocation`, `ClusterTopology` fields
  - File: `ractor_shell/src/monitor.rs` - already well documented

- [x] **Verify cargo doc builds without warnings**
  ```bash
  cargo doc --package ractor_shell --no-deps
  ```
  - No warnings or errors

- [x] **Update README.md**
  - Verified all build commands work
  - ractor_experiments references already removed in 1.1
  - Added "Requirements" section with Rust version, runtime, and workspace status
  - Badge for build status deferred (after CI integration)

- [x] **Add module-level documentation**
  - Added `//!` doc comments to: `lib.rs`, `introspection.rs`, `protocol.rs`, `messages.rs`, `commands/mod.rs`
  - `dynamic.rs`, `monitor.rs`, `completer.rs` already had module docs

### 1.3 Testing ✅

**Estimated Time**: 4-6 hours
**Status**: Complete

- [x] **Add unit tests for core functionality**
  - File: `ractor_shell/src/lib.rs`
    - Added 45 tests for `ShellCommand::parse_line` covering all commands, aliases, and error cases
    - Tests for prompt building included in integration tests
  - File: `ractor_shell/src/dynamic.rs`
    - Added test for DynamicMessage actor (test_dynamic_actor_ping)
  - Existing tests: completer (2), messages (3), monitor (1)
  - **Total unit tests: 52**

- [x] **Add integration tests**
  - Created: `ractor_shell/tests/integration_test.rs`
    - test_shell_state_and_prompt - Shell state creation and prompt building
    - test_spawn_and_query_actor - Actor spawning and registry queries
    - test_dynamic_message_ping - DynamicMessage support check
    - test_process_group_membership - Process group queries
    - test_call_response_success - RPC call success path
    - test_call_response_error - RPC call error path
    - test_cast_message - Cast message and state mutation
  - **Total integration tests: 7**

- [x] **Verify all examples run successfully**
  - `cargo run --example demo -p ractor_shell` ✓
  - `cargo run --example dynamic_actor -p ractor_shell` ✓
  - `cargo run --example monitoring_demo -p ractor_shell` ✓
  - All examples start, accept input, and exit cleanly

- [x] **Run tests in CI configuration**
  - `cargo test --package ractor_shell` ✓ (59 tests pass)
  - `cargo test --workspace` ✓ (all workspace tests pass)
  - `cargo clippy --package ractor_shell --tests --examples -- -D warnings` ✓
  - `cargo fmt --package ractor_shell -- --check` ✓

### 1.4 Dependencies Audit ✅

**Estimated Time**: 1-2 hours
**Status**: Complete

- [x] **Review ractor_shell dependencies**
  - File: `ractor_shell/Cargo.toml`
  - Workspace doesn't define `[workspace.dependencies]`, so versions managed per-crate
  - All dependency versions are reasonable and current
  - Checked `Cargo.lock` for duplicates - only transitive duplicates (not ractor_shell's concern)

- [x] **Verify minimal dependency set**
  All 12 dependencies are necessary and actively used:
  - `ractor` (workspace path) ✓ - Core actor framework
  - `ractor_cluster` (workspace path) ✓ - Cluster/remote support
  - `tokio` ✓ - Async runtime (required)
  - `clap` ✓ - CLI argument parsing (`--connect` flag)
  - `rustyline` ✓ - REPL readline support (essential)
  - `serde`/`serde_json` ✓ - JSON for dynamic messages (essential)
  - `tabled` ✓ - Table output formatting (essential for UX)
  - `colored` ✓ - Colored terminal output (integral to UX)
  - `anyhow` ✓ - Error handling (essential)
  - `dirs` ✓ - History file location (~/.ractor_shell_history)
  - `hostname` ✓ - System hostname for prompt
  - `chrono` ✓ - Timestamps in monitoring (23 usages)

- [x] **Consider feature flags for optional deps**
  **Decision: Not recommended** for colored/chrono:
  - `colored` is used throughout lib.rs, main.rs, monitor.rs - integral to UX
  - `chrono` is used 23 times in lib.rs and monitor.rs - integral to monitoring
  - Making them optional would require extensive conditional compilation
  - Both are fundamental to the shell's user experience
  - Added complexity would outweigh minimal dependency savings

### 1.5 CI/CD Integration ✅

**Estimated Time**: 1-2 hours
**Status**: Complete

- [x] **Update CI workflow to test ractor_shell**
  - File: `.github/workflows/ci.yaml`
  - Added test job for ractor_shell to matrix:
    ```yaml
    - name: Test ractor_shell
      package: ractor_shell
      # flags:
    ```

- [x] **Ensure ractor_shell doesn't break existing CI**
  - `cargo test` ✓ (default members still work)
  - `cargo clippy --all -- -D clippy::all -D warnings` ✓
  - `cargo fmt --all -- --check` ✓

- [x] **Document exclusion from default build**
  - Added "Workspace Structure" section to CONTRIBUTING.md
  - Documents ractor_shell as optional, not built by default
  - Includes build commands for working with ractor_shell

---

## Priority 2: Important (Should-Have for PR)

### 2.1 Code Improvements ✅

**Estimated Time**: 3-5 hours
**Status**: Complete

- [x] **Refactor large functions**
  - `execute()` method was already well-structured with individual `cmd_*` handlers
  - No further refactoring needed

- [x] **Improve error handling**
  - Added `thiserror` dependency for strongly-typed errors
  - Created `ractor_shell/src/error.rs` with `ShellError` enum
  - Replaced all `anyhow::Error` with specific `ShellError` variants
  - Error messages include actionable suggestions (e.g., "Use 'registry' command to list...")
  - Added `ShellError::messaging()` helper for generic `MessagingErr<T>` conversion
  - Documented best practices in `SKILLS.md`

- [x] **Reduce code duplication**
  - Analyzed table row structs - kept inline as they're context-specific
  - Added `ShellError::messaging()` helper to avoid repetitive error conversion
  - Documented patterns in `SKILLS.md`

- [x] **Add const for magic values**
  - Added `DEFAULT_RPC_TIMEOUT: Duration` (5 seconds)
  - Added `DEFAULT_NODE_SERVER_PORT: u16` (9100)
  - Added `DEFAULT_CLUSTER_COOKIE: &str`
  - Added `KNOWN_PROCESS_GROUPS: &[&str]`
  - Replaced all hardcoded timeouts with constant

### 2.2 Testing Enhancements ✅

**Estimated Time**: 2-3 hours
**Status**: Complete

- [x] **Add property-based tests**
  - Added `proptest` dependency
  - Created `tests/proptest_tests.rs` with 14 property tests
  - Tests command parsing with random inputs
  - Tests JSON parsing edge cases
  - Tests invariants (no panics, valid commands parse, etc.)

- [x] **Add benchmark tests**
  - Added `criterion` dependency
  - Created `benches/shell_bench.rs`
  - Benchmarks: command parsing (simple, with args, aliases)
  - Benchmarks: JSON parsing (various types)
  - Benchmarks: error cases (fast failure paths)
  - Run with: `cargo bench -p ractor_shell`

- [x] **Test coverage measurement**
  - Documented in `TESTING.md`
  - Instructions for using `cargo-tarpaulin`
  - Coverage targets: >90% parsing, >80% errors, >70% overall
  - Noted limitations for cluster operations

### 2.3 Documentation Improvements ✅

**Estimated Time**: 2-3 hours
**Status**: Complete

- [x] **Add architecture diagram to README**
  - Enhanced ASCII diagram showing shell interacts with ractor core
  - Shows IntrospectionActor flow with RPC path
  - Shows MonitorActor flow with supervision events
  - Added component flow diagrams (local, remote, monitor)

- [x] **Create ARCHITECTURE.md**
  - Created comprehensive `ractor_shell/ARCHITECTURE.md`
  - Explains design decisions (named actors only, process groups, dynamic messages, RPC)
  - Documents command flow (local vs remote paths)
  - Documents state management (node context, connections, topology cache)
  - Documents remote connection handling (sequence, protocol messages)

- [x] **Add more examples**
  - `cluster_node.rs`: Distributed shell connecting to cluster
  - `dynamic_actor.rs`: Custom actor with DynamicMessage (existing)
  - `monitoring_demo.rs`: Monitoring in production (existing)

### 2.4 Feature Completeness ✅

**Estimated Time**: 4-6 hours
**Status**: Complete

- [x] **Implement TODOs from code**
  - Found and addressed 1 TODO in `main.rs`:
    - Implemented `--connect` flag auto-connect feature
  - No remaining TODOs, FIXMEs, XXXs, or HACKs in codebase

- [x] **Review limitations documented in README**
  - Updated `README.md` "Current Limitations" section
  - Categorized limitations into:
    - "Addressable now (workarounds exist)" - DynamicMessage opt-in
    - "Requires ractor core API additions (Phase 2)" - named actors, supervision trees, monitoring
  - Added reference to ARCHITECTURE.md for design rationale

- [x] **Improve error messages**
  - All errors already use strongly-typed `ShellError` (from 2.1)
  - Enhanced error messages with actionable suggestions:
    - `UnknownCommand`: "Type 'help' to see available commands"
    - `MissingArgument`: "Type 'help {command}' for usage"
    - `UnknownSubcommand`: "Type 'help {parent}' for usage"
    - `JsonParseError`: Includes example JSON format
  - Existing errors already include suggestions (e.g., "Use 'registry' command...")

---

## Priority 3: Nice-to-Have (Can defer to follow-up PRs)

### 3.1 Advanced Features ✅

**Estimated Time**: 6-10 hours
**Status**: Complete

- [x] **Implement remote message sending**
  - Added `SendDynamicMessage` and `CallDynamicMessage` protocol messages
  - Implemented `send_dynamic_message_to_actor()` and `call_dynamic_message_to_actor()` in IntrospectionActor
  - Added `cmd_send_remote()` and `cmd_call_remote()` to ShellState
  - Remote `send` and `call` commands now work for DynamicMessage actors

- [x] **Add command history persistence**
  - Already implemented in `main.rs` - loads/saves `~/.ractor_shell_history`
  - History persists across shell sessions

- [x] **Add configuration file support**
  - Created `config.rs` module with `ShellConfig` struct
  - Loads from `~/.ractor_shell.toml` if present
  - Supports: `rpc_timeout_secs`, `node_server_port`, `cluster_cookie`, `auto_connect`, `history_file`, `max_history`
  - Added `toml` dependency for parsing
  - 11 unit tests for config module

- [x] **Improve tab completion**
  - Actor names, process groups, and node names were already dynamic
  - Added file path completion for `send-file`, `sendfile`, `sf`, `load`, and `l` commands
  - `complete_file_path()` helper lists directories and files with proper sorting

### 3.2 User Experience

**Estimated Time**: 2-4 hours

- [x] **Add colored output feature flag**
  - Added `--color` CLI flag with Auto/Always/Never modes
  - Auto-detects terminal via `std::io::IsTerminal`
  - Disable colors in non-interactive mode and when piped
  - Configurable via `~/.ractor_shell.toml` with `color = "auto|always|never"`

- [x] **Improve table formatting**
  - Created `table.rs` module with terminal-aware table formatting
  - `build_table()` uses terminal width for automatic column wrapping
  - Added `PaginatedResult` struct for pagination support
  - Added `format_pagination_info()` for display

- [x] **Add shell scripting mode**
  - Added `-x/--execute` flag for non-interactive execution
  - Added `--format json` for JSON error output
  - Proper exit codes (0 for success, 1 for failure)
  - Quiet mode suppresses startup messages in scripting mode
  - Example: `ractor-shell -x "actors" --format json`

### 3.3 Testing & CI

**Estimated Time**: 2-3 hours

- [x] **WASM compatibility check** - N/A by design
  - ractor_shell is a native CLI tool requiring terminal I/O, networking, and filesystem
  - Dependencies (rustyline, tokio/mio, colored) don't support WASM
  - Core ractor library supports WASM; the shell is intentionally native-only

- [x] **Local cluster test environment**
  - Created `scripts/test_cluster.sh` for multi-node testing
  - Starts N nodes on sequential ports with automatic cleanup
  - Logs output to `/tmp/ractor_node_*.log`
  - Docker not required for local testing

- [ ] **Add mutation testing** (optional/future)
  - Requires `cargo install cargo-mutants`
  - Can take significant time to run (re-runs tests many times)
  - Current test coverage: 71 unit tests + 14 proptest cases
  - Consider running before major releases

---

## Priority 4: Future Work (Phase 2 - Introspection APIs)

### 4.1 Core Ractor Changes Required

**See**: `docs/FORK_INTEGRATION_PLAN.md` Phase 2

These require changes to ractor core and should be separate PRs:

- [ ] **Add introspection module to ractor**
  - Feature-gated with `shell` or `introspection` feature
  - APIs: `get_all_actors()`, `get_actor_info()`, `get_supervision_tree()`
  - File: `ractor/src/introspection.rs`

- [ ] **Add supervision event subscription**
  - Global event broadcaster
  - Real-time monitoring

- [ ] **Add message queue depth API**
  - Expose channel queue length
  - Add `get_message_queue_len()` to ActorCell
  - Enable: message queue monitoring, backpressure detection

### 4.2 Investigate DynamicMessage vs ractor_cluster Serialization

**Priority**: Low (future consideration)
**Status**: Research needed

Currently, `ractor_shell` uses `DynamicMessage` (JSON-based, runtime-typed) for shell-to-actor communication, while `ractor_cluster` uses compile-time binary serialization for network messages. These are fundamentally different approaches:

| Aspect | DynamicMessage | ractor_cluster |
|--------|----------------|----------------|
| Type safety | Runtime (JSON) | Compile-time (typed enums) |
| Serialization | JSON text | Binary (BytesConvertable) |
| Coupling | Loose (any JSON) | Tight (must know message type) |
| Use case | Shell debugging | Distributed systems |

**Questions to investigate:**
- [ ] Could the shell work with cluster-serializable messages instead of requiring DynamicMessage opt-in?
- [ ] Would a message schema registry (mapping actor names → accepted message types) be feasible?
- [ ] Could actors expose message type info at runtime for shell introspection?
- [ ] Is there value in a hybrid approach where actors implement both interfaces?

**Trade-offs:**
- DynamicMessage provides flexibility for interactive debugging (arbitrary JSON)
- Cluster serialization provides type safety and efficiency
- Converging them would add complexity similar to gRPC reflection

**Conclusion from initial investigation:** The current design trades type safety for flexibility, which is appropriate for a debugging tool. However, this should be revisited if:
1. Users request type-safe shell interactions
2. A message schema registry becomes useful for other purposes
3. ractor_cluster gains reflection-like capabilities

### 4.3 Enhanced Introspection APIs (Erlang Parity)

**Estimated Time**: 8-12 hours (requires ractor core PRs)

These additions would bring ractor_shell closer to Erlang Observer capabilities:

- [ ] **Expose supervision tree publicly**
  - Make `get_children()` public on ActorCell (currently `pub(crate)`)
  - Make `try_get_supervisor()` public on ActorCell (currently `pub(crate)`)
  - Enable: true supervision tree visualization (not just process groups)
  - Erlang equivalent: `process_info(Pid, links)`, Observer supervision view

- [ ] **Add actor metrics API**
  - Messages processed count (similar to Erlang "reductions")
  - Actor uptime/start time
  - Enable: actor sorting by activity, identifying busy actors
  - Erlang equivalent: `process_info(Pid, reductions)`

- [ ] **Expose link/monitor relationships**
  - List monitors and monitored_by for an actor
  - Erlang equivalent: `process_info(Pid, monitors)`, `process_info(Pid, monitored_by)`

---

## Priority 5: Shell Enhancements (No Core Changes Required)

These features can be implemented in ractor_shell without modifications to ractor core.

### 5.1 Actor Console TUI (`top` command) ⭐ QUICK WIN

**Estimated Time**: 8-12 hours (Phase 1), expandable later
**Value**: HIGH - Visual real-time actor monitoring like Erlang's observer_cli
**Dependencies**: None (uses existing APIs + tracing spans)
**Status**: Phase 1 Complete ✅

Inspired by [observer_cli](https://github.com/zhongwencool/observer_cli) and [tokio-console](https://github.com/tokio-rs/console).

#### Phase 1: Quick Win (No Core Changes) ✅

- [x] **Add `ratatui` and `crossterm` dependencies**
  - TUI framework for terminal interface
  - Cross-platform terminal handling
  - Also added: `tracing-subscriber`, `dashmap`

- [x] **Create `top` command with basic TUI**
  - Full-screen terminal UI with ratatui
  - Auto-refresh (default 1s, configurable)
  - Exit with 'q' or Ctrl+C
  - New files: `tui/mod.rs`, `tui/app.rs`, `tui/ui.rs`, `tui/metrics.rs`
  ```
  ractor top - Actor Dashboard                    [q]uit [s]ort [/]filter
  ─────────────────────────────────────────────────────────────────────────
  ID      Name              Status     Uptime    PG Groups
  0.1     worker_1          Running    1m 23s    workers, demo_group
  0.2     worker_2          Running    1m 23s    workers
  0.3     supervisor        Running    1m 25s    supervisors
  0.0     shell_monitor     Running    1m 25s    -
  ─────────────────────────────────────────────────────────────────────────
  Actors: 4 total | 4 running | 0 stopped     Refresh: 1s     [?] help
  ```

- [x] **Data sources (available now)**
  - Actor list from `ractor::registry::registered()`
  - Actor status from `ActorCell::get_status()`
  - Process group membership from `ractor::pg`
  - Uptime: track locally when actors appear in registry

- [x] **Basic interactivity**
  - Sort by: name, ID, status, uptime, groups (press `s` to cycle, `S` to reverse)
  - Filter by name/ID pattern (press `/` to enter filter mode)
  - Arrow keys / j/k to navigate, g/G for top/bottom, PageUp/PageDown
  - Help overlay with `?`

- [x] **Create ActorMetricsCollector**
  - Collects metrics from registry and process groups
  - Tracks first-seen time for uptime calculation
  - Ready for tracing Layer integration in Phase 2

#### Phase 2: Enhanced Metrics (When Core APIs Available)

- [ ] **Add metrics columns (requires Priority 4.3)**
  - Msgs/sec: from `ActorCell::get_messages_processed()`
  - Queue depth: from `ActorCell::get_queue_depth()`
  - Precise uptime: from `ActorCell::get_start_time()`

- [ ] **Add supervision tree panel**
  - Show parent/child relationships (requires exposed tree)
  - Visual hierarchy like `pstree`

#### Phase 3: Full Dashboard

- [ ] **Multiple panels (tab-switchable)**
  - Actors panel (default)
  - System stats panel (totals, rates)
  - Process groups panel
  - Cluster panel (when connected to remote)

- [ ] **Sparkline graphs**
  - Message throughput over time
  - Actor count over time

- [ ] **Export/snapshot**
  - Dump current view to JSON
  - Screenshot to file

### 5.2 Tracing Support ✅

**Estimated Time**: 6-10 hours
**Value**: HIGH - One of the most valuable debugging tools in Erlang
**Status**: Complete

- [x] **Add `trace` command for message flow**
  - Trace actor communication patterns with glob pattern matching
  - Filter by actor name/pattern (e.g., `trace worker_*`, `trace *`)
  - Show trace output with timestamps and colored formatting
  - Implementation: Custom `tracing::Layer` that hooks into ractor's span instrumentation
  - Commands: `trace [pattern]`, `trace off`, aliases `tr`
  - Erlang equivalent: `dbg`, trace BIFs

- [x] **Add `trace-to-file` for persistent logging**
  - Export trace data to file: `trace-to-file <path> [pattern]`
  - Support Pretty, JSON, and Compact output formats
  - File output is additive to console output
  - Commands: `trace-to-file`, `tracefile`, alias `tf`

**New files:**
- `src/tracing/mod.rs` - Module exports
- `src/tracing/layer.rs` - ShellTracingLayer and TracingHandle
- `src/tracing/filter.rs` - TraceFilter with glob pattern matching
- `src/tracing/output.rs` - TraceEvent, TraceOutput, output formatting

### 5.3 Simple Watch Mode (Non-TUI Alternative)

**Estimated Time**: 2-3 hours
**Value**: MEDIUM - Lightweight alternative to full TUI

- [ ] **Add `watch` command with auto-refresh**
  - `watch actors` - refresh actor list every N seconds
  - `watch stats` - continuous system stats
  - `watch pg <group>` - monitor process group membership
  - Configurable refresh interval (default 2s)
  - Press 'q' or Ctrl+C to exit watch mode
  - Uses existing table output (no ratatui required)

### 5.4 Network/Cluster Diagnostics

**Estimated Time**: 3-4 hours
**Value**: MEDIUM - Important for distributed debugging

- [ ] **Enhance `nodes` command with connection stats**
  - Show connection duration (how long connected)
  - Show last activity timestamp
  - Show connection state (healthy/degraded)

- [ ] **Add `ping <node>` command**
  - Explicit latency measurement to remote node
  - Connection health check with round-trip time
  - Erlang equivalent: `net_adm:ping/1`

- [ ] **Add `netstat` command for cluster connections**
  - Show bytes sent/received per connection
  - Show pending message counts
  - Erlang equivalent: `inet:getstat/1`

### 5.5 Enhanced Actor Inspection

**Estimated Time**: 2-3 hours
**Value**: LOW-MEDIUM - Quality of life improvements

- [ ] **Add `whereis <name>` command (Erlang-style)**
  - Quick lookup of registered name to actor ID
  - Cleaner than filtering `registry` output
  - Erlang equivalent: `whereis/1`

- [ ] **Add `links <actor>` command placeholder**
  - Show what the actor is linked to (when 4.3 API available)
  - Show monitors/monitored_by relationships
  - Gracefully degrade when API not available

- [ ] **Rename `tree` to `pgtree` or clarify documentation**
  - Current `tree` shows process groups, not supervision trees
  - Add note explaining difference from Erlang supervision trees
  - Alternative: keep `tree` but add `supervtree` when 4.3 available

### 5.6 Shell UX Improvements

**Estimated Time**: 2-3 hours
**Value**: LOW - Polish items

- [ ] **Add `clear` command**
  - Clear terminal screen
  - Common shell convenience

- [ ] **Add `alias` command for user-defined aliases**
  - Let users define custom short commands
  - Persist in config file

- [ ] **Add `set` command for runtime configuration**
  - `set timeout 10` - change RPC timeout
  - `set refresh 5` - change watch refresh rate

### 5.7 Raft Debugging & Observability ⭐ HIGH PRIORITY

**Estimated Time**: 3-4 hours
**Value**: HIGH - Essential for observing and debugging Raft cluster behavior
**Status**: Not started

Currently, Raft messages are invisible in shell tracing because they're tunneled as JSON through `DynamicMessage` via the introspection actor, bypassing ractor_cluster's network-level tracing.

- [ ] **Enable Raft debug logging in shell tracing**
  - Add tracing instrumentation to Raft message handlers in `raft.rs`
  - Show RequestVote, VoteResponse, Heartbeat messages in `trace` output
  - Include term numbers, candidate/leader names, vote decisions
  - Consider adding a `trace raft` filter or `raft trace` command
  - File: `ractor_shell/src/raft.rs` (lines 296-362 handle incoming messages)

- [ ] **Add command to trigger fresh leader election**
  - New shell command: `raft election` or `raft stepdown`
  - Forces current leader to step down, triggering new election
  - Useful for observing election protocol in action
  - Implementation: Send a `DynamicMessage` to raft_node with `{"command": "stepdown"}` or similar
  - Add handler in `raft.rs` to transition leader → follower and clear election state
  - Alternative: `raft kill-leader` to stop the leader actor entirely

- [ ] **Add `raft status` command enhancements**
  - Show current term, voted_for, election generation
  - Show peer connection status (connected/disconnected)
  - Show time since last heartbeat received
  - Show vote tally during elections

**Why this matters:**
Without these features, debugging Raft behavior requires reading log files. The shell should provide real-time visibility into the consensus protocol, especially for educational/demo purposes.

---

## Erlang/OTP Feature Comparison

Reference for future development priorities:

| Feature | ractor_shell | Erlang Equivalent | Status |
|---------|-------------|-------------------|--------|
| Process listing | `actors`, `registry` | `processes()`, `registered()` | ✅ Implemented |
| Process groups | `pg list`, `pg members` | `pg:get_members/1` | ✅ Implemented |
| Basic info | `info` (ID, name, status) | `process_info/1` | ✅ Partial |
| Stop/Kill | `stop` | `exit/2` | ✅ Implemented |
| Remote connection | `connect`, `use` | `-remsh`, `net_adm:ping/1` | ✅ Implemented |
| Cluster topology | `cluster` commands | `nodes()`, observer | ✅ Implemented |
| Monitoring | `monitor`/`unmonitor` | `erlang:monitor/2` | ✅ Implemented |
| Message sending | `send`, `call` | Direct calls | ✅ DynamicMessage only |
| **Top/Dashboard TUI** | `top` | observer_cli | ✅ Phase 1 Complete |
| Message queue depth | - | `message_queue_len` | ❌ Needs core API (4.1) |
| Supervision trees | `tree` (pg only) | Observer supervision view | ⚠️ Needs core API (4.3) |
| Link inspection | - | `links`, `monitors` | ❌ Needs core API (4.3) |
| **Tracing** | `trace`, `trace-to-file` | `dbg`, trace BIFs | ✅ Complete |
| Live refresh (simple) | `watch` | - | ❌ Planned (5.3) |
| Memory/reductions | - | `memory`, `reductions` | ❌ Needs core API (4.3) |

---

## Checklist Before Opening PR

### Pre-Submission

- [x] All Priority 1 tasks completed (1.1 ✅, 1.2 ✅, 1.3 ✅, 1.4 ✅, 1.5 ✅)
- [x] All tests pass: `cargo test --workspace` (59 tests)
- [x] Clippy passes: `cargo clippy --all -- -D clippy::all -D warnings`
- [x] Rustfmt passes: `cargo fmt --all -- --check`
- [x] Documentation builds: `cargo doc --package ractor_shell --no-deps`
- [x] Examples run successfully (demo, dynamic_actor, monitoring_demo)
- [x] No ractor_experiments references remain
- [ ] Git history is clean (consider squashing commits)

### PR Description Template

```markdown
## Summary

Add `ractor_shell` as optional workspace member - an interactive REPL for debugging and observing Ractor actor systems, inspired by Erlang's `erl` shell.

## Features

- Interactive command-line interface with rustyline
- Actor introspection (registry, process groups, status)
- Remote node connections (ractor_cluster support)
- Dynamic JSON message sending to actors
- Actor lifecycle monitoring
- Cluster topology visualization
- Tab completion and command aliases

## Integration Approach

- Added as workspace member (NOT in default-members)
- Built explicitly: `cargo build -p ractor_shell`
- Zero impact on core ractor library
- Uses workspace paths for ractor/ractor_cluster dependencies

## Testing

- [ ] Unit tests for command parsing and core logic
- [ ] Integration tests with live actors
- [ ] All examples verified working
- [ ] CI updated to test ractor_shell

## Documentation

- [ ] Comprehensive README with usage examples
- [ ] Rustdoc comments on public APIs
- [ ] Architecture documentation
- [ ] Integration guide

## Breaking Changes

None - this is purely additive.

## Future Work

See `docs/FORK_INTEGRATION_PLAN.md` for roadmap including:
- Phase 2: Core introspection APIs
- Phase 3: Enhanced shell features

## Checklist

- [ ] Tests pass
- [ ] Clippy passes
- [ ] Rustfmt passes
- [ ] Documentation builds
- [ ] Examples verified
```

---

## Notes

### Project Conventions Observed

From analyzing ractor codebase:

1. **Formatting**: Standard rustfmt (see `rustfmt.toml`)
2. **Linting**: Strict clippy with `-D warnings`
3. **Testing**:
   - Unit tests in `#[cfg(test)] mod tests`
   - Integration tests in separate crate or `tests/` directory
   - Use `serial_test` for sequential tests if needed
4. **Documentation**:
   - Comprehensive rustdoc on public APIs
   - Examples in `examples/` directory
   - Markdown docs in `docs/` directory
5. **CI**:
   - Tests multiple feature combinations
   - Separate jobs for clippy, rustfmt, docs, benchmarks
   - WASM support checked

### Shell-Specific Considerations

1. **User-Facing Tool**: Error messages must be clear and helpful
2. **REPL Nature**: Must never panic (catch and display errors)
3. **Interactive**: Performance of command execution matters
4. **Dependencies**: Keep lightweight (shell is optional)

### Estimated Total Time

- **Priority 1 (Critical)**: 10-17 hours ✅ Complete
- **Priority 2 (Important)**: 11-16 hours ✅ Complete
- **Priority 3 (Nice-to-Have)**: 10-17 hours ✅ Mostly Complete
- **Priority 4 (Core Changes)**: 8-12 hours (requires ractor core PRs)
- **Priority 5 (Shell Enhancements)**: 23-35 hours (no core changes needed)
  - ✅ **5.1 Top/Dashboard TUI Phase 1: Complete**
  - 5.1 Phases 2-3: 4-8 hours (when core APIs available)
  - ✅ **5.2 Tracing: Complete**
  - 5.3 Simple Watch Mode: 2-3 hours (MEDIUM value)
  - 5.4 Network Diagnostics: 3-4 hours (MEDIUM value)
  - 5.5 Enhanced Inspection: 2-3 hours (LOW-MEDIUM value)
  - 5.6 UX Improvements: 2-3 hours (LOW value)
  - **5.7 Raft Debugging & Observability: 3-4 hours (HIGH value)**

**Recommended for initial PR**: Priority 1 + 2 = ✅ Complete

**Recommended next steps (post-PR)**:
1. ✅ **Priority 5.1 (`top` command) Phase 1** - Complete
2. ✅ **Priority 5.2 (Tracing)** - Complete
3. **Priority 5.7 (Raft Debugging)** - Enable Raft message tracing and election triggering
4. Priority 5.3 (Simple Watch Mode) - Lightweight auto-refresh without TUI
5. Priority 4.1 (Core APIs) - Unlocks metrics for `top` Phases 2-3

---

## Quick Start

To begin working through these tasks:

```bash
cd /Users/jhoward/git/rust_erlang/ractor

# 1. Format code
cargo fmt --package ractor_shell

# 2. Check for clippy issues
cargo clippy --package ractor_shell -- -D clippy::all -D warnings

# 3. Run tests
cargo test --package ractor_shell

# 4. Build docs
cargo doc --package ractor_shell --no-deps

# 5. Run examples
cargo run --example demo -p ractor_shell
```

Work through Priority 1 tasks first, then Priority 2 before opening PR.

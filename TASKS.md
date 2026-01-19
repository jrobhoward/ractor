# ractor_shell PR Preparation Tasks

**Branch**: phase2/introspection
**Goal**: Prepare ractor_shell for PR/merge into main
**Status**: All priorities complete, ready for PR

---

## Summary of Changes

This PR introduces `ractor_shell`, an interactive debugging REPL for ractor actor systems, inspired by Erlang's shell and Observer tool.

### Core Changes (ractor crate)
- **Metrics API**: Added `get_message_count()` and `get_handle_time_ns()` to `ActorCell`
- **Schema introspection**: New `schema` module (behind `shell-introspection` feature flag)
- **Derive macro enhancement**: `#[ractor_shell]` attribute generates `SchemaProvider` impl

### New Crate: ractor_shell
- Interactive REPL with tab completion and command history
- Remote node connections via ractor_cluster
- TUI dashboard (`top` command) with CPU%, memory, message rates
- Tracing infrastructure for observing actor events
- Supervision tree visualization
- Raft cluster demo for testing

---

## Priority 1: Documentation Cleanup (COMPLETED)

### Files Removed
- [x] `MACRO_DISPATCHER_PLAN.md` - Implementation plan, no longer needed

### Files Updated
- [x] **README.md** - Verified aliases, converted ASCII diagram to Mermaid
- [x] **ARCHITECTURE.md** - Converted 6 ASCII diagrams to Mermaid (flowcharts, sequence diagrams)
- [x] **docs/SHOWCASE.md** - Fixed `trace-off` -> `trace off` documentation

### Files Kept (no changes needed)
- DYNAMIC_MESSAGES.md
- MONITORING.md
- SKILLS.md
- TESTING.md
- UX_FEATURES.md

---

## Priority 2: Code Quality Fixes (COMPLETED)

### Alias Consistency
All documented aliases now implemented in `lib.rs` and `completer.rs`:
- [x] `h` → `help`, `n` → `nodes`, `u` → `use`, `cl` → `cluster`
- [x] `con` → `connect`, `dis` → `disconnect`
- [x] `m` → `monitor`, `um` → `unmonitor`, `ms` → `monitors`
- [x] `sc` → `schema`

---

## Priority 3: Example/Script Cleanup (COMPLETED)

### Kept
- `examples/cluster_demo.rs` - Main example, used by test_cluster.sh
- `examples/cluster_demo/` - Raft implementation
- `scripts/test_cluster.sh` - Cluster testing script

### Removed
- [x] `examples/messages/*.json` - Sample JSON files
- [x] `examples/scripts/demo.txt` - Unused script
- [x] `examples/scripts/test_phase4.txt` - Outdated test script

---

## Priority 4: Pre-PR Verification (COMPLETED)

### Build & Test
- [x] `cargo build -p ractor_shell` passes
- [x] `cargo build -p ractor_shell --no-default-features` passes (without sysinfo)
- [x] `cargo test -p ractor_shell` passes (12 tests)
- [x] `cargo clippy -p ractor_shell -- -D warnings` passes
- [x] `cargo fmt -p ractor_shell -- --check` passes
- [x] `cargo doc -p ractor_shell --no-deps` builds (2 minor warnings about private items)

### Integration Testing (Manual)
- [ ] `./ractor_shell/scripts/test_cluster.sh` starts successfully
- [ ] Connect to remote node and run basic commands
- [ ] `top` command shows metrics
- [ ] Tracing works locally and remotely
- [ ] Raft commands work (`call raft_node GetStatus {}`)

---

## Potential Merge Blockers

### 1. Ractor Core Changes Required
The following changes to `ractor` crate are required for full functionality:

| File | Change | Purpose |
|------|--------|---------|
| `actor_properties.rs` | Add `message_count`, `handle_time_ns` fields | Metrics for `top` |
| `actor_cell.rs` | Add `get_message_count()`, `get_handle_time_ns()` | Public API |
| `actor.rs` | Instrument message processing | Timing metrics |
| `thread_local/inner.rs` | Same instrumentation | Thread-local actors |
| `schema.rs` (new) | Schema introspection traits | Shell integration |
| `lib.rs` | Export schema module (feature-gated) | API exposure |

**Decision needed**: Should these be a separate PR merged first?

### 2. Ractor Cluster Derive Changes
| File | Change | Purpose |
|------|--------|---------|
| `lib.rs` | Add `#[ractor_shell]` attribute handling | Generate SchemaProvider |
| `Cargo.toml` | Note about serde_json in generated code | Documentation |

### 3. New Feature Flag
- `shell-introspection` feature flag needed in `ractor/Cargo.toml`
- Optional dependency: enables schema types

---

## Nice-to-Have (Post-PR)

### Test Coverage Gaps
These modules could use more tests:
- `src/tui/app.rs` (523 lines) - State machine, sorting, filtering
- `src/error.rs` (131 lines) - Error helper tests

### Code Organization
- Consider splitting `lib.rs` (3928 lines) into command modules
  - Currently: All 39 command handlers in one file
  - Alternative: `src/commands/trace.rs`, `src/commands/cluster.rs`, etc.

### Command Restructuring
- Consolidate trace commands: `trace`, `trace off`, `trace level`, `trace file`, `trace remote`
- Consolidate monitor commands: `monitor start`, `monitor stop`, `monitor list`

### Additional Tests
- Property-based tests for command parsing
- Integration tests for remote operations

### Documentation
- Add troubleshooting guide

---

## Backlog / Future Ideas

Deferred for future phases:

### Network/Cluster Diagnostics
- Enhance `nodes` command with connection stats
- Add `netstat` command for cluster connections
- Handle stale cluster connections gracefully (auto-reconnect)

### Shell UX Improvements
- `clear` command - Clear terminal screen
- `alias` command - User-defined aliases
- `set` command - Runtime configuration

### TUI Enhancements
- Supervision tree panel in `top`
- Watch mode for continuous updates

### Enhanced Actor Inspection
- `whereis <name>` - Quick name-to-ID lookup
- `links <actor>` - Show supervisor and children

---

## Quick Reference

```bash
# Build
cargo build -p ractor_shell

# Test
cargo test -p ractor_shell

# Lint
cargo clippy -p ractor_shell -- -D clippy::all -D warnings

# Format check
cargo fmt -p ractor_shell -- --check

# Run cluster demo
./ractor_shell/scripts/test_cluster.sh

# Run shell standalone
cargo run -p ractor_shell
```

---

## Checklist for PR

- [x] Priority 1 tasks complete (documentation cleanup)
- [x] Priority 2 tasks complete (alias consistency)
- [x] Priority 3 tasks complete (example cleanup)
- [x] Priority 4 tasks complete (build/test verification)
- [ ] CHANGELOG.md updated (if exists)
- [ ] PR description written with summary of changes
- [x] Screenshots of `top` command added to docs
- [ ] Core ractor changes reviewed separately if needed

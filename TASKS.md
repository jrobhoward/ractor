# ractor_shell Integration Tasks

**Branch**: feature/shell
**Goal**: Prepare ractor_shell for PR to upstream ractor repository
**Status**: Integration Complete, Pre-PR Cleanup In Progress (1.1 ✅, 1.2 ✅, 1.3 ✅, 1.4 ✅, 1.5 ✅)

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

### 2.1 Code Improvements

**Estimated Time**: 3-5 hours

- [ ] **Refactor large functions**
  - File: `ractor_shell/src/lib.rs`
    - `execute()` method is likely very long
    - Split into separate command handlers
    - Consider command pattern with trait

- [ ] **Improve error handling**
  - Replace generic `anyhow::Error` with specific error types where appropriate
  - Add context to errors using `.context()`
  - Ensure all errors are user-friendly (shell commands shouldn't panic)

- [ ] **Reduce code duplication**
  - Look for repeated patterns in command execution
  - Extract common functionality into helper functions
  - Example: Table formatting, actor lookups

- [ ] **Add const for magic values**
  - Port numbers, timeout values, buffer sizes
  - Replace string literals with constants

### 2.2 Testing Enhancements

**Estimated Time**: 2-3 hours

- [ ] **Add property-based tests**
  - Consider using `proptest` or `quickcheck`
  - Test command parsing with random inputs
  - Test JSON parsing edge cases

- [ ] **Add benchmark tests**
  - File: `ractor_shell/benches/shell_bench.rs`
  - Benchmark command parsing
  - Benchmark actor lookups
  - Benchmark table formatting

- [ ] **Test coverage measurement**
  ```bash
  cargo install cargo-tarpaulin
  cargo tarpaulin --package ractor_shell
  ```
  - Aim for >70% coverage on core logic
  - Document uncovered areas

### 2.3 Documentation Improvements

**Estimated Time**: 2-3 hours

- [ ] **Add architecture diagram to README**
  - Show how shell interacts with ractor core
  - Show IntrospectionActor flow
  - Show MonitorActor flow

- [ ] **Create ARCHITECTURE.md**
  - Explain design decisions
  - Document command flow
  - Document state management
  - Document remote connection handling

- [ ] **Add more examples**
  - Example: Distributed shell connecting to cluster
  - Example: Custom actor with DynamicMessage
  - Example: Monitoring in production

### 2.4 Feature Completeness

**Estimated Time**: 4-6 hours

- [ ] **Implement TODOs from code**
  ```bash
  rg "TODO|FIXME|XXX|HACK" ractor_shell/src/
  ```
  - Address or document all TODOs
  - Prioritize user-facing functionality
  - File issues for future work

- [ ] **Review limitations documented in README**
  - File: `ractor_shell/README.md`, section "Current Limitations"
  - Address what's feasible without core ractor changes
  - Clearly document what requires Phase 2 (introspection APIs)

- [ ] **Improve error messages**
  - User-facing errors should be clear and actionable
  - Include suggestions for fixes
  - Example: "Actor 'foo' not found. Try 'actors' to list all actors."

---

## Priority 3: Nice-to-Have (Can defer to follow-up PRs)

### 3.1 Advanced Features

**Estimated Time**: 6-10 hours

- [ ] **Implement remote message sending**
  - Currently returns "not yet implemented"
  - Files: `ractor_shell/src/lib.rs` lines 604-608, 712-716
  - Requires understanding remote actor serialization

- [ ] **Add command history persistence**
  - Save command history to `~/.ractor_shell_history`
  - Load on startup
  - Already using `dirs` crate

- [ ] **Add configuration file support**
  - `~/.ractor_shell.toml` or similar
  - Configure aliases, default timeouts, etc.

- [ ] **Improve tab completion**
  - File: `ractor_shell/src/completer.rs`
  - Add completion for actor names dynamically
  - Add completion for process group names
  - Add completion for file paths (send-file, load commands)

### 3.2 User Experience

**Estimated Time**: 2-4 hours

- [ ] **Add colored output feature flag**
  - Make `colored` crate optional
  - Disable colors in CI or when piped
  - Check `atty` or `is_terminal()`

- [ ] **Improve table formatting**
  - Auto-adjust column widths based on terminal size
  - Add pagination for large result sets
  - Add sorting options

- [ ] **Add shell scripting mode**
  - Non-interactive mode for automation
  - Output JSON instead of tables
  - Exit codes for success/failure

### 3.3 Testing & CI

**Estimated Time**: 2-3 hours

- [ ] **Add WASM compatibility check**
  - Verify ractor_shell builds for WASM target
  - May need feature flags to disable rustyline on WASM

- [ ] **Add Docker test environment**
  - Test cluster connections in isolated environment
  - Multi-node testing

- [ ] **Add mutation testing**
  - Use `cargo-mutants` to find untested code paths

---

## Priority 4: Future Work (Phase 2 - Introspection APIs)

### 4.1 Core Ractor Changes Required

**See**: `docs/FORK_INTEGRATION_PLAN.md` Phase 2

These require changes to ractor core and should be separate PRs:

- [ ] **Add introspection module to ractor**
  - Feature-gated with `shell` feature
  - APIs: `get_all_actors()`, `get_actor_info()`, `get_supervision_tree()`
  - File: `ractor/src/introspection.rs`

- [ ] **Add supervision event subscription**
  - Global event broadcaster
  - Real-time monitoring

- [ ] **Add message queue depth API**
  - Expose channel queue length

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

- **Priority 1 (Critical)**: 10-17 hours
- **Priority 2 (Important)**: 11-16 hours
- **Priority 3 (Nice-to-Have)**: 10-17 hours

**Recommended for initial PR**: Priority 1 + 2.1 + 2.2 = ~15-25 hours

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

# ractor_shell Testing Conventions

This document describes the testing conventions for the `ractor_shell` workspace member.

## File Organization

### Separate Test Files

Tests are isolated to separate files rather than inline `mod tests` blocks:

```
src/
├── foo.rs              # Main module code
├── foo/
│   └── foo_tests.rs    # Tests for foo module
├── bar.rs
└── bar/
    └── bar_tests.rs
```

### Module Import

The test module is imported at the **end** of the parent file:

```rust
// foo.rs

// ... module implementation ...

#[cfg(test)]
mod foo_tests;
```

### Test File Naming

Test files follow the pattern: `{parent_module_name}_tests.rs`

- `monitor.rs` → `monitor/monitor_tests.rs`
- `dynamic.rs` → `dynamic/dynamic_tests.rs`
- `lib.rs` → `lib_tests.rs` (special case: lives in `src/` directly)

## Test File Structure

### File Header

Each test file starts with:

```rust
#![allow(non_snake_case)]

use crate::module_name::*;  // Import from parent module
// Additional imports as needed
```

The `#![allow(non_snake_case)]` directive permits the triple-underscore naming convention.

## Test Naming Convention

Tests follow a structured naming pattern with **triple underscores** as separators:

```
subject_under_test___condition___expected_result
```

### Components

1. **subject_under_test**: The function, method, or component being tested
2. **condition**: The specific scenario or input condition
3. **expected_result**: What should happen (the assertion)

### Examples

```rust
#[test]
fn parse_line___empty_input___returns_empty_command_error() { ... }

#[test]
fn parse_line___valid_actors_command___returns_actors_variant() { ... }

#[test]
fn format___actor_started___contains_expected_output() { ... }

#[test]
fn execute___unknown_command___returns_error() { ... }
```

### Guidelines

- Use lowercase with single underscores within each component
- Use triple underscores (`___`) only as separators between components
- Be specific but concise
- The test name should read as a specification

## Test Body Structure

Tests follow the **Arrange-Act-Assert** pattern, separated by blank lines (no comments):

```rust
#[test]
fn parse_line___help_command___returns_help_variant() {
    let input = "help";

    let result = ShellCommand::parse_line(input);

    assert!(matches!(result, Ok(ShellCommand::Help { command: None })));
}
```

### Structure

1. **Arrange**: Set up test data and preconditions (first block)
2. **Act**: Execute the code under test (second block)
3. **Assert**: Verify the results (third block)

### Guidelines

- Separate sections with a single blank line
- Do NOT add `// Arrange`, `// Act`, `// Assert` comments
- Keep each section focused and minimal
- For simple tests, sections may be combined if clarity is maintained

## Async Tests

For async tests, use `#[tokio::test]`:

```rust
#[tokio::test]
async fn execute___actors_command___lists_registered_actors() {
    let state = ShellState::new().await.unwrap();

    let result = state.execute(ShellCommand::Actors).await;

    assert!(result.is_ok());
}
```

## Test Assertions

Prefer specific assertions over generic ones:

```rust
// Good - specific
assert_eq!(result, expected_value);
assert!(matches!(result, Ok(ShellCommand::Help { .. })));

// Avoid - too generic
assert!(result.is_ok());  // Only use when the Ok value doesn't matter
```

## Integration Tests

Integration tests live in `tests/` directory and test public APIs:

```
ractor_shell/
├── src/
└── tests/
    └── integration_test.rs
```

Integration tests follow the **same naming conventions** as unit tests:

```rust
#![allow(non_snake_case)]

#[tokio::test]
async fn ShellState___new_and_build_prompt___initializes_correctly() {
    let state = ShellState::new().await;

    assert!(state.is_ok());
    // ...
}

#[tokio::test]
async fn DynamicMessage___call_with_ping_command___returns_success_response() {
    // Arrange
    let (actor_ref, _handle) = Actor::spawn(...).await.unwrap();

    // Act
    let result = actor_ref.call(...).await;

    // Assert
    assert!(result.is_ok());
    actor_ref.stop(None);
}
```

**Key points for integration tests:**
- Add `#![allow(non_snake_case)]` at file top
- Use the same `subject___condition___expected` naming pattern
- Use Arrange-Act-Assert with whitespace separation
- Clean up actors with `.stop(None)` after each test

## Property-Based Tests

Property-based tests use `proptest` to generate random inputs and verify invariants.

Location: `tests/proptest_tests.rs`

```rust
#![allow(non_snake_case)]

use proptest::prelude::*;

proptest! {
    #[test]
    fn parse_line___any_string___does_not_panic(input in ".*") {
        let _ = ShellCommand::parse_line(&input);
    }

    #[test]
    fn parse_line___valid_command___parses_successfully(
        cmd in prop_oneof![Just("actors"), Just("registry"), Just("nodes")]
    ) {
        let result = ShellCommand::parse_line(cmd);

        prop_assert!(result.is_ok());
    }
}
```

**Run property tests:**
```bash
cargo test --package ractor_shell --test proptest_tests
```

## Benchmarks

Benchmarks use `criterion` and live in `benches/shell_bench.rs`.

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_command_parsing(c: &mut Criterion) {
    c.bench_function("parse_actors", |b| {
        b.iter(|| ShellCommand::parse_line(black_box("actors")))
    });
}

criterion_group!(benches, bench_command_parsing);
criterion_main!(benches);
```

**Run benchmarks:**
```bash
cargo bench -p ractor_shell
```

Benchmark results are saved to `target/criterion/` with HTML reports.

## Test Coverage

Measure test coverage using `cargo-tarpaulin`:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage for ractor_shell
cargo tarpaulin --package ractor_shell --out Html

# View report
open tarpaulin-report.html
```

**Coverage targets:**
- Core command parsing: >90%
- Error handling: >80%
- Overall: >70%

**Note:** Some code paths (remote connections, cluster operations) are difficult to test without a full cluster setup and may have lower coverage.

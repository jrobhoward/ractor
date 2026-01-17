# ractor_shell Development Skills & Best Practices

This document captures best practices and conventions specific to the ractor_shell workspace member.

## Error Handling

### Use `thiserror` for Error Types

Prefer `thiserror` over `anyhow` for defining error types. This provides:
- Strongly-typed errors with clear semantics
- Automatic `Display` and `Error` trait implementations
- Better error composition and matching
- Clearer API contracts

**Example:**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("Actor '{0}' not found. Try 'actors' to list all actors.")]
    ActorNotFound(String),

    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    #[error("Connection failed to {host}:{port}: {source}")]
    ConnectionFailed {
        host: String,
        port: u16,
        #[source]
        source: std::io::Error,
    },

    #[error("RPC call timed out after {0:?}")]
    RpcTimeout(std::time::Duration),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
}
```

**Guidelines:**
- Create specific error variants for each failure mode
- Include actionable context in error messages (e.g., "Try 'actors' to list all actors")
- Use `#[from]` for automatic conversion from underlying errors
- Use `#[source]` to preserve error chains
- Keep `anyhow` for top-level error aggregation in main/examples only

### Error Message Style

Error messages should be:
- **User-friendly**: Written for humans, not developers
- **Actionable**: Include suggestions when possible
- **Contextual**: Include relevant values (actor names, ports, etc.)

```rust
// Good
#[error("Actor '{name}' not found in registry. Use 'registry' command to list registered actors.")]

// Bad
#[error("NotFound")]
```

## Code Organization

### Command Handlers

Each shell command should have its own handler function to keep `execute()` manageable:

```rust
impl ShellState {
    pub async fn execute(&mut self, cmd: ShellCommand) -> Result<(), ShellError> {
        match cmd {
            ShellCommand::Actors => self.handle_actors().await,
            ShellCommand::Registry => self.handle_registry().await,
            // ...
        }
    }

    async fn handle_actors(&self) -> Result<(), ShellError> {
        // Implementation
    }
}
```

### Constants

Define constants for magic values:

```rust
/// Default timeout for RPC calls
pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Default port for cluster connections
pub const DEFAULT_CLUSTER_PORT: u16 = 9000;

/// Maximum history entries to retain
pub const MAX_HISTORY_ENTRIES: usize = 1000;
```

### Table Formatting with `tabled`

Define row structs inline within command handlers for clarity:

```rust
async fn cmd_actors(&self) -> ShellResult<()> {
    #[derive(Tabled)]
    struct ActorRow {
        #[tabled(rename = "Name")]
        name: String,
        #[tabled(rename = "ID")]
        id: String,
        #[tabled(rename = "Status")]
        status: String,
    }

    let rows: Vec<ActorRow> = /* ... */;
    let table = Table::new(rows).to_string();
    println!("{}", table);
}
```

This keeps the display format close to where it's used and makes the code self-documenting.

### Handling Generic Messaging Errors

When working with ractor's `MessagingErr<T>` which is generic over the message type,
use the helper method instead of `#[from]` conversion:

```rust
// In error.rs
impl ShellError {
    /// Create a MessagingError from any MessagingErr type.
    pub fn messaging<T: std::fmt::Debug>(err: ractor::MessagingErr<T>) -> Self {
        ShellError::MessagingError(format!("{:?}", err))
    }
}

// Usage in lib.rs
actor_ref
    .cast(MyMessage::DoSomething)
    .map_err(ShellError::messaging)?;
```

## Testing

See [TESTING.md](./TESTING.md) for complete testing conventions.

**Key points:**
- Tests are isolated to separate `*_tests.rs` files (not inline `mod tests` blocks)
- Test naming: `subject_under_test___condition___expected_result`
- Use Arrange-Act-Assert pattern separated by whitespace (no comments)
- Unit test command parsing exhaustively
- Integration test with real actors where possible
- Test error paths, not just happy paths
- Use `#[tokio::test]` for async tests involving actors

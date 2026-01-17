# Phase 4 Testing Guide - File & Message Input

This guide shows how to test the message sending and script execution features.

## What's New in Phase 4

Phase 4 adds dynamic message construction and script execution:
- **Enhanced `send` command** - Parse JSON messages for cast operations
- **Enhanced `call` command** - Parse JSON messages for RPC operations
- **`send-file` command** - Send messages from JSON files
- **`load` command** - Execute shell commands from script files
- **JSON message parsing** - Flexible message construction with serde_json

## Prerequisites

- Build the project: `cargo build --all`
- Example files are in `examples/messages/` and `examples/scripts/`

## Understanding the Type Safety Limitation

**Important**: Due to Rust's strong type system, the shell cannot directly send arbitrary messages to actors. Each actor has a specific message enum type (e.g., `PingPongMessage`), and the shell would need to know this type at compile time to call `ActorRef<T>::cast()`.

The Phase 4 implementation:
- ✅ Parses JSON messages correctly
- ✅ Validates JSON syntax
- ✅ Shows what would be sent
- ⚠️ Cannot actually send to strongly-typed actors

For real message sending, actors would need to implement a common dynamic message interface.

## Test Procedure

### Start the Shell

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments/ractor_shell
cargo run --example demo
```

This starts the demo with sample actors.

## Testing Commands

### 1. Test JSON Message Parsing (send)

```
ractor@local > send demo_actor_1 {"Ping": ["shell", 1]}
```

Expected output:
```
send demo_actor_1 <- {"Ping": ["shell", 1]}

⚠ Limitation: Type-safe message sending requires knowing the actor's message type at compile time.

  • The shell cannot send arbitrary messages to strongly-typed actors.
  • For this to work, actors must accept a common dynamic message type.
  • See the documentation for implementing a DynamicMessage trait.

Would send to actor demo_actor_1 (ID: 0.0)
Message: {
  "Ping": [
    "shell",
    1
  ]
}

Future Enhancement:
  When actors implement a common message interface, this will:
  - Actually send the message
  - Confirm delivery
  - Show any errors
```

### 2. Test Invalid JSON

```
ractor@local > send demo_actor_1 this is not json
```

Expected output:
```
send demo_actor_1 <- this is not json

Parse error: Invalid JSON. Message must be valid JSON or a simple string.

Expected JSON format. Examples:
  send ping_pong {"Ping": ["shell", 1]}
  send my_actor {"DoWork": ["task1"]}
```

### 3. Test RPC Message Parsing (call)

```
ractor@local > call demo_actor_1 {"GetStats": null}
```

Expected output:
```
call demo_actor_1 <- {"GetStats": null}

⚠ Limitation: Type-safe RPC requires knowing the actor's message type at compile time.

  • The shell cannot make arbitrary RPC calls to strongly-typed actors.
  • RPC messages must be marked with #[rpc] in the message enum.
  • Actors must implement a common dynamic RPC interface for shell interaction.

Would call actor demo_actor_1 (ID: 0.0)
Message: {
  "GetStats": null
}

Future Enhancement:
  When actors implement a common RPC interface, this will:
  - Send the RPC message
  - Wait for the reply
  - Display the response
  - Handle timeouts gracefully
```

### 4. Test send-file Command

```
ractor@local > send-file demo_actor_1 examples/messages/ping.json
```

Expected output:
```
send-file demo_actor_1 <- examples/messages/ping.json

✓ Read 32 bytes from examples/messages/ping.json

send demo_actor_1 <- {
  "Ping": ["shell_user", 1]
}

[... same limitation message as send command ...]
```

### 5. Test Complex JSON Message

```
ractor@local > send-file demo_actor_1 examples/messages/complex_message.json
```

This demonstrates sending a complex nested JSON structure.

### 6. Test load Command

```
ractor@local > load examples/scripts/demo.txt
```

Expected output:
```
load examples/scripts/demo.txt

✓ Loaded script from examples/scripts/demo.txt

• Executing 6 commands...

▸ [1] help

Available Commands:

  Local Introspection:
  ...

▸ [2] registry

Registered Actors (3):
  ...

▸ [3] stats

System Statistics
...

▸ [4] pg list

Process Groups:
  ...

▸ [5] tree

Process Group Tree
...

✓ Script execution complete
```

### 7. Test Script with Error Handling

Create a script with an invalid command:

```bash
echo "registry" > /tmp/test_script.txt
echo "invalid_command" >> /tmp/test_script.txt
echo "stats" >> /tmp/test_script.txt
```

Then run it:

```
ractor@local > load /tmp/test_script.txt
```

The script should stop at the invalid command and show an error.

### 8. Test Help for New Commands

```
ractor@local > help send-file
```

Expected output:
```
send-file <actor> <file>
  Send a message from a JSON file to an actor

Usage:
  send-file <actor_name> <file_path>

Example:
  send-file ping_pong messages/ping.json
```

```
ractor@local > help load
```

Expected output:
```
load <script>
  Execute shell commands from a script file

Usage:
  load <script_path>

Example:
  load scripts/setup.txt

Script Format:
  - One command per line
  - Lines starting with # are comments
  - Empty lines are ignored
```

## Key Features Demonstrated

### JSON Message Parsing
- Validates JSON syntax before attempting to send
- Pretty-prints parsed messages
- Provides clear error messages for invalid JSON
- Shows examples of correct JSON format

### send-file Command
- Reads JSON messages from files
- Convenient for complex messages
- Reuses the send command logic
- Shows file size and path

### load Command
- Executes multiple commands from a script
- Supports comments (lines starting with #)
- Skips empty lines
- Shows progress with line numbers
- Stops on first error
- Uses Box::pin for recursive async calls

### Script Features
- Comment support with `#`
- Empty line filtering
- Sequential execution
- Error handling with early termination
- Progress indicators

## Creating Your Own Message Files

### Simple Enum Variant

```json
{
  "Ping": ["sender_name", 42]
}
```

### Struct-like Message

```json
{
  "field1": "value1",
  "field2": 123,
  "nested": {
    "inner": "data"
  }
}
```

### Array Message

```json
["item1", "item2", "item3"]
```

### Simple String

```json
"Hello, actor!"
```

## Creating Shell Scripts

Example script (`examples/scripts/inspect_cluster.txt`):

```
# Cluster inspection script

# Show local stats
stats

# List all actors
actors

# Show process groups
pg list

# Display cluster topology
cluster

# Show cluster-wide actors
cluster actors
```

Run it with:
```
ractor@local > load examples/scripts/inspect_cluster.txt
```

## Future Enhancements

Once actors implement a common dynamic message interface, the shell will be able to:

1. **Actually send messages** instead of just parsing them
2. **Receive and display replies** from RPC calls
3. **Show delivery confirmation** for cast messages
4. **Handle errors** from the actor system
5. **Support message timeouts** and retries

### Example Dynamic Message Trait

```rust
#[async_trait]
pub trait DynamicMessage: Message {
    async fn handle_dynamic(&self, msg: serde_json::Value) -> Result<serde_json::Value>;
}
```

Actors implementing this trait would be able to receive arbitrary JSON messages from the shell.

## Phase 4 Complete!

You've successfully:
- ✓ Enhanced send/call commands with JSON parsing
- ✓ Created the send-file command for file-based messages
- ✓ Implemented the load command for script execution
- ✓ Understood the type safety limitations and future path
- ✓ Learned to create message files and shell scripts

Next phases:
- Phase 3: Monitoring and live event streams
- Phase 7: Tab completion and UX polish
- Future: Dynamic message interface for real message sending

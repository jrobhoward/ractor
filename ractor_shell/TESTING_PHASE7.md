# Phase 7 Testing Guide - UX Polish

This guide shows how to test the user experience enhancements in the ractor shell.

## What's New in Phase 7

Phase 7 adds UX polish to make the shell delightful to use:
- **Tab completion** - Auto-complete commands, actors, process groups, and nodes
- **Command aliases** - Short forms like `a`, `r`, `i`, `s`, `c`
- **Smart hints** - See what aliases expand to
- **Better help** - Information about shell features

## Prerequisites

- Build the project: `cargo build --examples`
- Terminal with readline support (most modern terminals)

## Test Procedure

### Start the Demo

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments/ractor_shell
cargo run --example demo
```

Or for dynamic message support:
```bash
cargo run --example dynamic_actor
```

## Testing Tab Completion

### 1. Command Completion

Type partial commands and press TAB:

```
ractor@local > he<TAB>
→ help

ractor@local > regi<TAB>
→ registry

ractor@local > pg<TAB>
→ pg
```

If multiple matches exist, press TAB multiple times to cycle through them:

```
ractor@local > s<TAB>
→ send | send-file | sendfile | stats | stop
```

### 2. Subcommand Completion

Test subcommand completion:

```
ractor@local > pg <TAB>
→ list | members

ractor@local > pg m<TAB>
→ members

ractor@local > cluster <TAB>
→ nodes | groups | actors
```

### 3. Actor Name Completion

After running `registry` to see actors:

```
ractor@local > registry
Registered Actors (3):
  demo_actor_1 → 0.0
  demo_actor_2 → 0.1
  demo_actor_3 → 0.2

ractor@local > info dem<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3

ractor@local > info demo_actor_<TAB>
→ demo_actor_1

ractor@local > stop demo_<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3
```

### 4. Process Group Completion

Test process group completion:

```
ractor@local > pg members <TAB>
→ ping_pong | ractor_shell_introspection | demo_group | dynamic_group

ractor@local > pg members demo<TAB>
→ demo_group
```

### 5. Node Completion (with cluster)

First connect to a node:

```
# In another terminal, start node_b:
cd /Users/jhoward/git/rust_erlang/ractor_experiments
cargo run --bin node_b
```

Then test node completion:

```
ractor@local > connect 127.0.0.1:9002
...

ractor@local > use <TAB>
→ local | 127.0.0.1:9002

ractor@local > disconnect <TAB>
→ 127.0.0.1:9002
```

## Testing Command Aliases

### 1. Basic Aliases

Try the short forms:

```
# Instead of "actors"
ractor@local > a
+--------------+-----+---------+
| Name         | ID  | Status  |
+--------------+-----+---------+
| demo_actor_1 | 0.0 | Running |
| demo_actor_2 | 0.1 | Running |
| demo_actor_3 | 0.2 | Running |
+--------------+-----+---------+

# Instead of "registry"
ractor@local > r
Registered Actors (3):
  demo_actor_1 → 0.0
  demo_actor_2 → 0.1
  demo_actor_3 → 0.2

# Instead of "quit"
ractor@local > q
Goodbye!
```

### 2. Aliases with Arguments

Test aliases that take arguments:

```
# info alias
ractor@local > i demo_actor_1
Actor: demo_actor_1
  ID:     0.0
  Status: Running
  Name:   demo_actor_1

# send alias (with dynamic actor)
ractor@local > s dynamic_actor {"command": "increment"}
✓ Actor supports dynamic messages
✓ Message sent to dynamic_actor

# call alias
ractor@local > c dynamic_actor {"command": "get_counter"}
✓ Actor supports dynamic messages
• Making RPC call...

✓ RPC call successful

Response:
{
  "count": 1
}
```

### 3. Alias List

View all available aliases:

```
ractor@local > help

...

  Shell Features:
  • Use TAB to auto-complete commands, actors, and groups
  • Command history with UP/DOWN arrows
  • Aliases: a=actors, r=registry, i=info, s=send, c=call, q=quit
```

## Testing Command History

### 1. Basic Navigation

Type several commands, then navigate:

```
ractor@local > registry
ractor@local > actors
ractor@local > stats

# Press UP arrow
→ shows "stats"

# Press UP arrow again
→ shows "actors"

# Press DOWN arrow
→ shows "stats"
```

### 2. History Persistence

```
# Exit the shell
ractor@local > quit

# Restart
cargo run --example demo

# Press UP arrow - your previous commands should be there!
```

### 3. Search History (Ctrl+R)

```
# Type Ctrl+R, then type search term
(reverse-i-search)`reg': registry

# Press Enter to execute
```

## Testing Help System

### 1. General Help

```
ractor@local > help

Available Commands:

  Local Introspection:
  help [command]  Show this help message
  actors            List all actors
  ...

  Shell Features:
  • Use TAB to auto-complete commands, actors, and groups
  • Command history with UP/DOWN arrows
  • Aliases: a=actors, r=registry, i=info, s=send, c=call, q=quit
```

### 2. Command-Specific Help

```
ractor@local > help send-file
send-file <actor> <file>
  Send a message from a JSON file to an actor

Usage:
  send-file <actor_name> <file_path>

Example:
  send-file ping_pong messages/ping.json
```

## Testing Complete Workflows

### Workflow 1: Quick Actor Inspection

```
# Use aliases and tab completion for speed
ractor@local > r<TAB>           # Completes to "registry"
ractor@local > registry         # Shows all actors
ractor@local > i de<TAB>        # Completes to "demo_actor_"
ractor@local > i demo_actor_1   # Shows info

# Or even faster with alias
ractor@local > i dem<TAB>       # Completes to "demo_actor_1"
```

### Workflow 2: Message Sending with Completion

```
# Start dynamic_actor example
cargo run --example dynamic_actor

# Use tab completion and aliases
ractor@local > s dy<TAB>        # Completes to "dynamic_actor"
ractor@local > s dynamic_actor {"command": "increment"}
✓ Message sent to dynamic_actor

ractor@local > c dy<TAB>        # Completes to "dynamic_actor"
ractor@local > c dynamic_actor {"command": "get_counter"}
{
  "count": 1
}
```

### Workflow 3: Cluster Navigation

```
# Connect to nodes, then use tab completion
ractor@local > connect 127.0.0.1:9002
ractor@local > use 127<TAB>     # Completes to "127.0.0.1:9002"
ractor@127.0.0.1:9002 > r       # Alias for "registry"
ractor@127.0.0.1:9002 > use l<TAB>  # Completes to "local"
ractor@local >
```

## Key Features Demonstrated

### Tab Completion Works For:
- ✅ Command names
- ✅ Subcommands (pg, cluster)
- ✅ Actor names (info, send, call, stop)
- ✅ Process group names (pg members)
- ✅ Node names (use, disconnect)
- ✅ Help topics (help)

### Aliases Work For:
- ✅ `a` → actors
- ✅ `r` → registry
- ✅ `i` → info
- ✅ `s` → send
- ✅ `c` → call
- ✅ `sf` → send-file
- ✅ `l` → load
- ✅ `q` → quit

### History Features:
- ✅ UP/DOWN arrows
- ✅ Ctrl+R reverse search
- ✅ Persistence across sessions
- ✅ Smart navigation

## Tips for Testing

### Test Edge Cases

1. **Empty Completion**
   ```
   ractor@local > xyz<TAB>
   # Should show no completions
   ```

2. **Multiple Completions**
   ```
   ractor@local > s<TAB><TAB><TAB>
   # Should cycle through: send, send-file, sendfile, stats, stop
   ```

3. **Context-Aware Completion**
   ```
   ractor@local > help s<TAB>
   # Should complete to: send, send-file, sendfile, stats, stop

   ractor@local > pg members s<TAB>
   # Should show no completions (no process groups starting with 's')
   ```

### Verify Completion Updates

1. Connect to a node - node completion should update
2. Spawn new actors - actor completion should update
3. Join process groups - group completion should update

## Keyboard Shortcuts to Test

- **Ctrl+A** - Move to start of line
- **Ctrl+E** - Move to end of line
- **Ctrl+K** - Delete to end of line
- **Ctrl+U** - Delete to start of line
- **Ctrl+W** - Delete previous word
- **Ctrl+L** - Clear screen (if supported)
- **Ctrl+C** - Cancel line (don't exit)
- **Ctrl+D** - Exit (when line is empty)

## Expected Behavior

### Tab Completion Should:
- Show relevant suggestions based on context
- Complete unambiguous matches automatically
- Cycle through matches when pressed multiple times
- Work mid-word (e.g., `dem<TAB>` → `demo_actor_1`)

### Aliases Should:
- Work exactly like their full command equivalents
- Accept the same arguments
- Show hints about what they expand to (future)

### History Should:
- Remember all commands from current session
- Persist across sessions
- Allow navigation with arrow keys
- Support reverse search

## Common Issues

### Tab Doesn't Complete
- Make sure your terminal supports readline
- Try pressing TAB twice
- Check that the match is unambiguous

### Aliases Don't Work
- Check spelling (`a` not `A`)
- Verify it's in the alias list (`help` to see all)
- Try the full command name

### History Not Persisting
- Check file permissions in ~/.local/share/ractor_shell/ (or equivalent)
- Verify rustyline history is enabled

## Phase 7 Complete!

You've successfully tested:
- ✓ Tab completion for all command types
- ✓ Command aliases for faster typing
- ✓ Command history with navigation
- ✓ Enhanced help system with UX features
- ✓ Context-aware suggestions

The shell is now polished and delightful to use!

Next phases:
- Phase 3: Monitoring and live event streams (next priority)
- Future: Advanced features based on user feedback

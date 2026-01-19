# UX Features - Phase 7

The ractor shell includes several user experience enhancements to make it more pleasant and efficient to use.

## Tab Completion

Press **TAB** to auto-complete:

### Command Completion

Type partial commands and press TAB:
```
ractor@local > he<TAB>
→ help

ractor@local > regi<TAB>
→ registry

ractor@local > clus<TAB>
→ cluster
```

### Subcommand Completion

Complete subcommands for multi-part commands:
```
ractor@local > pg <TAB>
→ list | members

ractor@local > cluster <TAB>
→ nodes | groups | actors
```

### Actor Name Completion

Auto-complete actor names when using commands that require them:
```
ractor@local > info dem<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3

ractor@local > send dynamic_<TAB>
→ dynamic_actor

ractor@local > stop demo_actor_<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3
```

### Process Group Completion

Complete process group names after `pg members`:
```
ractor@local > pg members demo<TAB>
→ demo_group

ractor@local > pg members ping<TAB>
→ ping_pong
```

### Node Name Completion

Complete node names for remote operations:
```
ractor@local > use <TAB>
→ local | 127.0.0.1:9002 | 127.0.0.1:9001

ractor@local > disconnect <TAB>
→ 127.0.0.1:9002 | 127.0.0.1:9001
```

## Command Aliases

Use short aliases for frequently used commands:

| Alias | Full Command | Example |
|-------|--------------|---------|
| `a` | `actors` | `a` lists all actors |
| `r` | `registry` | `r` shows registry |
| `i` | `info` | `i demo_actor_1` |
| `s` | `send` | `s my_actor {"cmd": "test"}` |
| `c` | `call` | `c my_actor {"cmd": "get"}` |
| `sf` | `send-file` | `sf my_actor msg.json` |
| `l` | `load` | `l script.txt` |
| `q` | `quit` | `q` exits the shell |

Examples:
```bash
# Instead of typing "registry"
ractor@local > r

# Instead of typing "info demo_actor_1"
ractor@local > i demo_actor_1

# Instead of typing "send dynamic_actor"
ractor@local > s dynamic_actor {"command": "increment"}
```

## Command History

Navigate through previous commands using arrow keys:

- **UP Arrow** - Previous command
- **DOWN Arrow** - Next command
- **Ctrl+R** - Reverse search through history
- **Ctrl+L** - Clear screen (on supported terminals)

Your command history persists across sessions!

## Hints and Inline Help

The shell provides hints as you type:

```
ractor@local > a (actors)
               ↑
            Shows what alias expands to
```

## Error Messages

Clear, helpful error messages guide you:

```
ractor@local > pg
✗ pg requires a subcommand: list, members

ractor@local > info nonexistent
✗ Actor 'nonexistent' not found in registry

ractor@local > send demo_actor_1 not json
✗ Parse error: Invalid JSON. Message must be valid JSON or a simple string.

Expected JSON format. Examples:
  send ping_pong {"Ping": ["shell", 1]}
  send my_actor {"DoWork": ["task1"]}
```

## Context-Aware Completions

Tab completion knows the context of your command:

### After "info", "send", "call", "stop"
Only suggests registered actor names

### After "pg members"
Only suggests known process groups

### After "use" or "disconnect"
Only suggests connected node names

### After "help"
Only suggests valid command names

### After "cluster"
Only suggests cluster subcommands (nodes, groups, actors)

## Smart Defaults

The shell makes intelligent assumptions:

- `cluster` defaults to `cluster nodes` if no subcommand given
- `use local` switches back to local mode
- Empty commands are ignored (just press Enter)

## Visual Feedback

Commands provide clear visual feedback:

### Success Messages
```
✓ Message sent to demo_actor_1
✓ Connected to node at 127.0.0.1:9002
✓ RPC call successful
```

### Warnings
```
⚠ This actor doesn't support dynamic messages
⚠ Topology discovery timeout (non-fatal)
```

### Errors
```
✗ Actor 'foo' not found in registry
✗ RPC call timed out after 5 seconds
```

### Progress Indicators
```
• Making RPC call...
• Executing 6 commands...
▸ [1] registry
▸ [2] stats
```

## Tips and Tricks

### Quick Actor Info
```bash
# Type just the first letter and tab to see all actors starting with that letter
ractor@local > info d<TAB>
→ demo_actor_1 | demo_actor_2 | demo_actor_3 | dynamic_actor
```

### Script Execution
```bash
# Use tab completion for script filenames (if supported by your terminal)
ractor@local > load examples/scripts/<TAB>
→ demo.txt | test_phase4.txt
```

### Keyboard Shortcuts

Rustyline supports many Emacs-style shortcuts:

- **Ctrl+A** - Move to beginning of line
- **Ctrl+E** - Move to end of line
- **Ctrl+K** - Delete from cursor to end of line
- **Ctrl+U** - Delete from cursor to beginning of line
- **Ctrl+W** - Delete word before cursor
- **Ctrl+C** - Cancel current line (don't exit)
- **Ctrl+D** - Exit shell (when line is empty)

### Multi-Word Completion

If TAB shows multiple matches, press TAB again to cycle through them:
```
ractor@local > info demo<TAB>
               demo_actor_1
<TAB>          demo_actor_2
<TAB>          demo_actor_3
<TAB>          demo_actor_1  (cycles back)
```

## Implementation Details

Tab completion is powered by:
- **rustyline** - Readline-style line editing library
- **Custom Completer** - Context-aware completion logic
- **Dynamic Updates** - Completion data refreshed before each command

The completer tracks:
- All registered actor names
- Known process groups
- Connected node names
- Valid commands and subcommands

This ensures suggestions are always up-to-date with the current system state.

## Future Enhancements

Planned improvements for future releases:

1. **File Path Completion**
   - Complete file paths for `send-file` and `load` commands
   - Navigate directories with TAB

2. **Syntax Highlighting**
   - Highlight commands in green
   - Highlight errors in red as you type
   - Dim suggestions and hints

3. **Inline Documentation**
   - Show command syntax hints as you type
   - Display parameter descriptions

4. **Smart History**
   - Filter history by command type (Ctrl+R improvements)
   - Save frequently used commands as snippets

5. **Multi-Line Editing**
   - Support for complex JSON messages over multiple lines
   - Better handling of nested structures

## See Also

- `help` - List all commands
- `TESTING_PHASE7.md` - Testing guide for UX features
- rustyline documentation: https://docs.rs/rustyline/

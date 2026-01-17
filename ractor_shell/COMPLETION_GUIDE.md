# Tab Completion Configuration Guide

This guide explains how tab completion works in the ractor shell and how to customize it.

## How It Works

### Architecture

```
┌──────────────────────────────────────────────────┐
│              User Types Command                   │
│                      ↓                            │
│              Presses TAB Key                      │
└──────────────────────────────────────────────────┘
                       ↓
┌──────────────────────────────────────────────────┐
│              Rustyline Library                    │
│   - Captures TAB event                            │
│   - Calls Completer::complete()                   │
│   - Passes: line, cursor position, context        │
└──────────────────────────────────────────────────┘
                       ↓
┌──────────────────────────────────────────────────┐
│              ShellHelper (Our Code)               │
│   - Analyzes command context                      │
│   - Determines what type of completion needed     │
│   - Fetches relevant candidates                   │
│   - Filters by current input                      │
│   - Returns matches                               │
└──────────────────────────────────────────────────┘
                       ↓
┌──────────────────────────────────────────────────┐
│              Rustyline Display                    │
│   - Shows matches if multiple                     │
│   - Auto-completes if single match                │
│   - Beeps if no matches                           │
└──────────────────────────────────────────────────┘
```

### Current Completion Logic

The completer uses context-aware matching:

1. **Parse the line**: Split into words, identify command
2. **Determine context**: What comes next?
   - After empty line → command names
   - After "info" → actor names
   - After "pg members" → process group names
   - After "use" → node names + "local"
3. **Get candidates**: Fetch from cached state
4. **Filter**: Match against current input (prefix match)
5. **Return**: Position and list of matches

## Configuring Completion Behavior

### Option 1: Change Matching Strategy

**Current:** Prefix matching only
```rust
if cmd.starts_with(prefix) {
    candidates.push(...)
}
```

**Alternative: Case-insensitive**
```rust
if cmd.to_lowercase().starts_with(&prefix.to_lowercase()) {
    candidates.push(...)
}
```

To enable, edit `src/completer.rs` line 129:
```rust
// Before:
if cmd.starts_with(prefix) {

// After:
if cmd.to_lowercase().starts_with(&prefix.to_lowercase()) {
```

**Alternative: Fuzzy matching**
```rust
// Match characters in order, but not necessarily consecutive
// "da1" matches "demo_actor_1"
if fuzzy_match(prefix, cmd) {
    candidates.push(...)
}
```

See `src/completer_custom.rs` for implementation.

### Option 2: Change Display Format

**Current:** Simple list
```
demo_actor_1
demo_actor_2
demo_actor_3
```

**Alternative: With descriptions**
```
demo_actor_1    - Demo actor instance 1
demo_actor_2    - Demo actor instance 2
demo_actor_3    - Demo actor instance 3
```

To enable, modify the `Pair` creation:
```rust
Pair {
    display: format!("{:<20} - {}", name, description),
    replacement: name.clone(),
}
```

**Alternative: Multi-column**
```
demo_actor_1    demo_actor_2    demo_actor_3
test_actor_1    test_actor_2    test_actor_3
```

Use the `MultiColumnDisplay` trait from `completer_custom.rs`.

### Option 3: Change Sorting/Ranking

**Current:** Alphabetical order
```rust
candidates.sort_by(|a, b| a.display.cmp(&b.display));
```

**Alternative: Most recently used first**
```rust
// Track usage in ShellHelper
pub struct ShellHelper {
    pub recent_actors: Vec<String>,  // New field
    // ... existing fields
}

// In complete(), sort by recency:
candidates.sort_by(|a, b| {
    let a_recent = self.recent_actors.iter().position(|r| r == &a.display);
    let b_recent = self.recent_actors.iter().position(|r| r == &b.display);

    match (a_recent, b_recent) {
        (Some(ai), Some(bi)) => ai.cmp(&bi),  // Both recent, sort by recency
        (Some(_), None) => std::cmp::Ordering::Less,  // a is recent
        (None, Some(_)) => std::cmp::Ordering::Greater,  // b is recent
        (None, None) => a.display.cmp(&b.display),  // Neither recent, alphabetical
    }
});
```

**Alternative: Frequency-based**
```rust
pub struct ShellHelper {
    pub usage_counts: HashMap<String, usize>,  // New field
}

// Sort by usage count
candidates.sort_by(|a, b| {
    let a_count = self.usage_counts.get(&a.display).unwrap_or(&0);
    let b_count = self.usage_counts.get(&b.display).unwrap_or(&0);
    b_count.cmp(a_count)  // Descending order
});
```

### Option 4: Add File Path Completion

**For `load` and `send-file` commands:**

```rust
"load" | "send-file" | "sendfile" => {
    if parts.len() >= 2 {
        // Complete file paths
        let candidates = self.complete_file_path(current_word);
        return Ok((word_start, candidates));
    }
}

fn complete_file_path(&self, partial: &str) -> Vec<Pair> {
    // See completer_custom.rs for full implementation
    std::fs::read_dir(".")
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| {
                    let name = e.ok()?.file_name().to_str()?.to_string();
                    if name.starts_with(partial) {
                        Some(Pair { display: name.clone(), replacement: name })
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
```

### Option 5: Change Completion Trigger

**Current:** TAB key only

**Alternative: Multiple triggers**

Rustyline supports configuration:
```rust
let config = Config::builder()
    .completion_type(CompletionType::Circular)  // Cycle through matches
    .edit_mode(EditMode::Emacs)  // Or Vi mode
    .auto_add_history(true)
    .build();

let mut rl = Editor::with_config(config)?;
```

**Completion Types:**
- `CompletionType::Circular` - TAB cycles through matches
- `CompletionType::List` - TAB shows list, repeated TAB completes first

### Option 6: Add Contextual Hints

**Show usage hints as you type:**

Modify the `Hinter` impl in `completer.rs`:

```rust
impl Hinter for ShellHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        if pos < line.len() {
            return None;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "send" if parts.len() == 1 => {
                Some(" <actor> <json_message>".to_string())
            }
            "info" if parts.len() == 1 => {
                Some(" <actor_name>".to_string())
            }
            "pg" if parts.len() == 1 => {
                Some(" list | members <group>".to_string())
            }
            _ => None
        }
    }
}
```

Result:
```
ractor@local > send  <actor> <json_message>
                    ↑ hint appears in gray
```

### Option 7: Syntax Highlighting

**Color commands as you type:**

Modify the `Highlighter` impl:

```rust
impl Highlighter for ShellHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Cow::Borrowed(line);
        }

        let commands = Self::commands();
        let first_word = parts[0];

        if commands.contains(&first_word) {
            // Highlight valid commands in green
            let highlighted = line.replacen(
                first_word,
                &format!("\x1b[32m{}\x1b[0m", first_word),
                1
            );
            return Cow::Owned(highlighted);
        } else if Self::aliases().iter().any(|(alias, _)| *alias == first_word) {
            // Highlight aliases in cyan
            let highlighted = line.replacen(
                first_word,
                &format!("\x1b[36m{}\x1b[0m", first_word),
                1
            );
            return Cow::Owned(highlighted);
        }

        Cow::Borrowed(line)
    }
}
```

## Practical Configuration Examples

### Example 1: Enable Fuzzy Matching

Edit `src/completer.rs`:

```rust
// Add this function to ShellHelper
impl ShellHelper {
    fn fuzzy_match(pattern: &str, candidate: &str) -> bool {
        let mut chars = candidate.chars();
        for p_char in pattern.chars() {
            loop {
                match chars.next() {
                    Some(c) if c == p_char => break,
                    Some(_) => continue,
                    None => return false,
                }
            }
        }
        true
    }
}

// Change matching logic (around line 130)
for cmd in Self::commands() {
    if Self::fuzzy_match(prefix, cmd) {  // Changed from starts_with
        candidates.push(Pair {
            display: cmd.to_string(),
            replacement: cmd.to_string(),
        });
    }
}
```

Now typing `str` will match `stats` and `tree` (has t, r, in order).

### Example 2: Smart Sorting by Recency

```rust
// Add to ShellHelper struct
pub struct ShellHelper {
    pub actor_names: Vec<String>,
    pub process_groups: Vec<String>,
    pub node_names: Vec<String>,
    pub recent_completions: Vec<String>,  // NEW
}

// Update when completion is used
impl ShellHelper {
    pub fn record_completion(&mut self, completion: String) {
        // Keep last 10
        self.recent_completions.retain(|c| c != &completion);
        self.recent_completions.insert(0, completion);
        self.recent_completions.truncate(10);
    }
}

// In complete(), sort with recency boost
candidates.sort_by(|a, b| {
    let a_pos = self.recent_completions.iter().position(|r| r == &a.display);
    let b_pos = self.recent_completions.iter().position(|r| r == &b.display);

    match (a_pos, b_pos) {
        (Some(ai), Some(bi)) => ai.cmp(&bi),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.display.cmp(&b.display),
    }
});
```

### Example 3: Add Command History-Aware Completion

```rust
// Modify completion to suggest from history
fn complete_from_history(&self, prefix: &str, history: &dyn History) -> Vec<Pair> {
    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();

    // Iterate history in reverse (most recent first)
    for entry in history.iter().rev() {
        if entry.starts_with(prefix) && seen.insert(entry.to_string()) {
            candidates.push(Pair {
                display: entry.to_string(),
                replacement: entry.to_string(),
            });
        }
    }

    candidates
}
```

### Example 4: Configure Rustyline Behavior

In your REPL setup (`examples/demo.rs`):

```rust
use rustyline::config::{Config, CompletionType, EditMode};
use rustyline::Editor;

let config = Config::builder()
    .completion_type(CompletionType::Circular)  // TAB cycles through
    .edit_mode(EditMode::Emacs)  // Emacs keybindings
    .auto_add_history(true)  // Automatically add to history
    .history_ignore_space(true)  // Don't save commands starting with space
    .max_history_size(1000)?  // Keep 1000 commands
    .build();

let mut rl = Editor::with_config(config)?;
```

**CompletionType Options:**
- `Circular` - TAB cycles: `a` → `actors` → `a` → `actors`
- `List` - TAB shows all, second TAB completes first
- Custom logic in your Completer

**EditMode Options:**
- `Emacs` - Ctrl+A (start), Ctrl+E (end), etc.
- `Vi` - Vim-style modal editing

## Testing Your Changes

After modifying completion behavior:

```bash
# Rebuild
cargo build --examples

# Test
cargo run --example demo

# Try various completions
ractor@local > he<TAB>
ractor@local > info dem<TAB>
ractor@local > pg <TAB>
```

## Performance Considerations

**Current Implementation:** O(n) filtering where n = number of candidates

**For Large Actor Lists (1000+):**

1. **Use a Trie for prefix matching:**
```rust
use std::collections::HashMap;

struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_terminal: bool,
    value: Option<String>,
}

// Insert: O(m) where m = string length
// Query: O(m) instead of O(n*m)
```

2. **Lazy loading:**
```rust
// Only fetch candidates when needed
fn get_actor_names_lazy(&self) -> Vec<String> {
    // Check cache timestamp
    // Only query registry if stale
}
```

3. **Limit results:**
```rust
candidates.truncate(50);  // Show max 50 matches
```

## See Also

- Rustyline documentation: https://docs.rs/rustyline/
- `src/completer.rs` - Current implementation
- `src/completer_custom.rs` - Advanced examples
- `examples/demo.rs` - Usage example

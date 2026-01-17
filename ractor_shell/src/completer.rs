//! Tab completion for the ractor shell
//!
//! Provides context-aware tab completion for commands, actor names,
//! process groups, node names, and file paths.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;
use std::path::Path;

/// Helper for rustyline that provides tab completion
#[derive(Default)]
pub struct ShellHelper {
    /// Cached actor names for completion
    pub actor_names: Vec<String>,
    /// Cached process group names for completion
    pub process_groups: Vec<String>,
    /// Cached node names for completion
    pub node_names: Vec<String>,
}

impl ShellHelper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update actor names for completion
    pub fn update_actors(&mut self, names: Vec<String>) {
        self.actor_names = names;
    }

    /// Update process group names for completion
    pub fn update_groups(&mut self, names: Vec<String>) {
        self.process_groups = names;
    }

    /// Update node names for completion
    pub fn update_nodes(&mut self, names: Vec<String>) {
        self.node_names = names;
    }

    /// Get all available commands
    fn commands() -> Vec<&'static str> {
        vec![
            "help",
            "actors",
            "registry",
            "pg",
            "info",
            "send",
            "call",
            "send-file",
            "sendfile",
            "load",
            "stop",
            "connect",
            "disconnect",
            "nodes",
            "use",
            "cluster",
            "stats",
            "tree",
            "monitor",
            "unmonitor",
            "monitors",
            "exit",
            "quit",
        ]
    }

    /// Get command aliases
    fn aliases() -> Vec<(&'static str, &'static str)> {
        vec![
            ("a", "actors"),
            ("r", "registry"),
            ("i", "info"),
            ("s", "send"),
            ("c", "call"),
            ("sf", "send-file"),
            ("l", "load"),
            ("q", "quit"),
            ("exit", "quit"),
        ]
    }

    /// Get subcommands for a command
    fn subcommands(cmd: &str) -> Vec<&'static str> {
        match cmd {
            "pg" => vec!["list", "members"],
            "cluster" => vec!["nodes", "groups", "actors"],
            "use" => vec!["local"],
            _ => vec![],
        }
    }

    /// Complete file paths
    fn complete_file_path(prefix: &str) -> Vec<Pair> {
        let mut candidates = Vec::new();

        // Handle empty prefix - show current directory contents
        let (dir_path, file_prefix) = if prefix.is_empty() {
            (Path::new("."), "")
        } else {
            let path = Path::new(prefix);
            // If ends with separator or is a directory, list its contents
            if prefix.ends_with('/') || prefix.ends_with(std::path::MAIN_SEPARATOR) || path.is_dir()
            {
                (path, "")
            } else {
                // Otherwise, complete partial filename in the parent directory
                (
                    path.parent().unwrap_or(Path::new(".")),
                    path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
                )
            }
        };

        // Read directory contents
        if let Ok(entries) = std::fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if file_name.starts_with(file_prefix) {
                        let is_dir = entry.path().is_dir();
                        let display = if is_dir {
                            format!("{}/", file_name)
                        } else {
                            file_name.clone()
                        };

                        // Build the full replacement path
                        let replacement = if prefix.is_empty() || prefix == "." {
                            display.clone()
                        } else if prefix.ends_with('/')
                            || prefix.ends_with(std::path::MAIN_SEPARATOR)
                        {
                            format!("{}{}", prefix, display)
                        } else if let Some(parent) = Path::new(prefix).parent() {
                            if parent == Path::new("") {
                                display.clone()
                            } else {
                                format!("{}/{}", parent.to_str().unwrap_or(""), display)
                            }
                        } else {
                            display.clone()
                        };

                        candidates.push(Pair {
                            display,
                            replacement,
                        });
                    }
                }
            }
        }

        // Sort directories first, then by name
        candidates.sort_by(|a, b| {
            let a_is_dir = a.display.ends_with('/');
            let b_is_dir = b.display.ends_with('/');
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.display.cmp(&b.display),
            }
        });

        candidates
    }
}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let line = &line[..pos];
        let parts: Vec<&str> = line.split_whitespace().collect();

        // If empty or just whitespace, suggest commands
        if parts.is_empty() {
            let commands: Vec<Pair> = Self::commands()
                .into_iter()
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();
            return Ok((0, commands));
        }

        let first_word = parts[0];
        let word_start = line.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);

        // Complete first word (command or alias)
        if parts.len() == 1 && !line.ends_with(' ') {
            let prefix = first_word;
            let mut candidates = Vec::new();

            // Match commands
            for cmd in Self::commands() {
                if cmd.starts_with(prefix) {
                    candidates.push(Pair {
                        display: cmd.to_string(),
                        replacement: cmd.to_string(),
                    });
                }
            }

            // Match aliases
            for (alias, _) in Self::aliases() {
                if alias.starts_with(prefix) {
                    candidates.push(Pair {
                        display: alias.to_string(),
                        replacement: alias.to_string(),
                    });
                }
            }

            candidates.sort_by(|a, b| a.display.cmp(&b.display));
            return Ok((word_start, candidates));
        }

        // Complete subcommands or arguments based on the command
        let current_word = parts.last().copied().unwrap_or("");
        let current_word = if line.ends_with(' ') {
            ""
        } else {
            current_word
        };

        match first_word {
            "pg" => {
                if parts.len() == 2 && !line.ends_with(' ') {
                    // Complete pg subcommands
                    let candidates: Vec<Pair> = Self::subcommands("pg")
                        .into_iter()
                        .filter(|s| s.starts_with(current_word))
                        .map(|s| Pair {
                            display: s.to_string(),
                            replacement: s.to_string(),
                        })
                        .collect();
                    return Ok((word_start, candidates));
                } else if parts.len() >= 2 && parts[1] == "members" {
                    // Complete process group names after "pg members"
                    let candidates: Vec<Pair> = self
                        .process_groups
                        .iter()
                        .filter(|g| g.starts_with(current_word))
                        .map(|g| Pair {
                            display: g.clone(),
                            replacement: g.clone(),
                        })
                        .collect();
                    return Ok((word_start, candidates));
                }
            }
            "cluster" => {
                if parts.len() == 2 && !line.ends_with(' ') {
                    // Complete cluster subcommands
                    let candidates: Vec<Pair> = Self::subcommands("cluster")
                        .into_iter()
                        .filter(|s| s.starts_with(current_word))
                        .map(|s| Pair {
                            display: s.to_string(),
                            replacement: s.to_string(),
                        })
                        .collect();
                    return Ok((word_start, candidates));
                }
            }
            "info" | "stop" | "send" | "call" | "monitor" | "unmonitor" | "i" | "s" | "c" => {
                // Complete actor names (first argument)
                if parts.len() <= 2 {
                    let candidates: Vec<Pair> = self
                        .actor_names
                        .iter()
                        .filter(|a| a.starts_with(current_word))
                        .map(|a| Pair {
                            display: a.clone(),
                            replacement: a.clone(),
                        })
                        .collect();
                    return Ok((word_start, candidates));
                }
            }
            "send-file" | "sendfile" | "sf" => {
                if parts.len() <= 2 {
                    // First argument: actor name
                    let candidates: Vec<Pair> = self
                        .actor_names
                        .iter()
                        .filter(|a| a.starts_with(current_word))
                        .map(|a| Pair {
                            display: a.clone(),
                            replacement: a.clone(),
                        })
                        .collect();
                    return Ok((word_start, candidates));
                } else if parts.len() == 3 || (parts.len() == 2 && line.ends_with(' ')) {
                    // Second argument: file path
                    let candidates = Self::complete_file_path(current_word);
                    return Ok((word_start, candidates));
                }
            }
            "load" | "l" => {
                // Complete file paths
                let candidates = Self::complete_file_path(current_word);
                return Ok((word_start, candidates));
            }
            "use" => {
                // Complete node names or "local"
                let mut candidates: Vec<Pair> = self
                    .node_names
                    .iter()
                    .filter(|n| n.starts_with(current_word))
                    .map(|n| Pair {
                        display: n.clone(),
                        replacement: n.clone(),
                    })
                    .collect();

                if "local".starts_with(current_word) {
                    candidates.push(Pair {
                        display: "local".to_string(),
                        replacement: "local".to_string(),
                    });
                }

                return Ok((word_start, candidates));
            }
            "disconnect" => {
                // Complete connected node names
                let candidates: Vec<Pair> = self
                    .node_names
                    .iter()
                    .filter(|n| n.starts_with(current_word))
                    .map(|n| Pair {
                        display: n.clone(),
                        replacement: n.clone(),
                    })
                    .collect();
                return Ok((word_start, candidates));
            }
            "help" => {
                // Complete command names for help
                let candidates: Vec<Pair> = Self::commands()
                    .into_iter()
                    .filter(|cmd| cmd.starts_with(current_word))
                    .map(|cmd| Pair {
                        display: cmd.to_string(),
                        replacement: cmd.to_string(),
                    })
                    .collect();
                return Ok((word_start, candidates));
            }
            _ => {}
        }

        Ok((word_start, vec![]))
    }
}

impl Hinter for ShellHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        // Show hints for partial commands
        if pos < line.len() {
            return None;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let first_word = parts[0];

        // Check if it's an alias and show what it expands to
        for (alias, expansion) in Self::aliases() {
            if first_word == alias {
                return Some(format!(" ({})", expansion));
            }
        }

        None
    }
}

impl Highlighter for ShellHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // Basic highlighting would go here
        // For now, just return the line as-is
        // In the future, could use ANSI codes to color commands, etc.
        Cow::Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
        false
    }
}

impl Validator for ShellHelper {}

impl Helper for ShellHelper {}

/// Update the completer with current shell state
pub fn update_completer_state(
    helper: &mut ShellHelper,
    actor_names: Vec<String>,
    process_groups: Vec<String>,
    node_names: Vec<String>,
) {
    helper.update_actors(actor_names);
    helper.update_groups(process_groups);
    helper.update_nodes(node_names);
}

/// Get well-known process groups for completion
pub fn get_known_process_groups() -> Vec<String> {
    vec![
        "ping_pong".to_string(),
        "ractor_shell_introspection".to_string(),
        "demo_group".to_string(),
        "dynamic_group".to_string(),
    ]
}

#[cfg(test)]
mod completer_tests;

//! Tab completion for the ractor shell
//!
//! Provides context-aware tab completion for commands, actor names,
//! process groups, and node names.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;

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
        let current_word = if line.ends_with(' ') { "" } else { current_word };

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
            "info" | "stop" | "send" | "call" | "send-file" | "sendfile" | "monitor" | "unmonitor"
            | "i" | "s" | "c" | "sf" => {
                // Complete actor names
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
mod tests {
    use super::*;

    #[test]
    fn test_command_completion() {
        let helper = ShellHelper::new();
        let (pos, candidates) = helper.complete("he", 2, &Context::new(&rustyline::history::DefaultHistory::new())).unwrap();

        assert_eq!(pos, 0);
        assert!(candidates.iter().any(|c| c.display == "help"));
    }

    #[test]
    fn test_pg_subcommand_completion() {
        let helper = ShellHelper::new();
        let (pos, candidates) = helper.complete("pg m", 4, &Context::new(&rustyline::history::DefaultHistory::new())).unwrap();

        assert!(candidates.iter().any(|c| c.display == "members"));
    }
}

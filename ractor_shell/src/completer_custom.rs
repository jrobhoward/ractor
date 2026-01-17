//! Advanced Tab Completion Customization Examples
//!
//! This file demonstrates various ways to customize tab completion behavior.

use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use crate::completer::ShellHelper;

// ============================================================================
// EXAMPLE 1: Case-Insensitive Completion
// ============================================================================

pub trait CaseInsensitiveCompletion {
    fn complete_case_insensitive(&self, line: &str, candidates: &[String]) -> Vec<Pair>;
}

impl CaseInsensitiveCompletion for ShellHelper {
    fn complete_case_insensitive(&self, line: &str, candidates: &[String]) -> Vec<Pair> {
        let line_lower = line.to_lowercase();
        candidates
            .iter()
            .filter(|c| c.to_lowercase().starts_with(&line_lower))
            .map(|c| Pair {
                display: c.clone(),
                replacement: c.clone(),
            })
            .collect()
    }
}

// ============================================================================
// EXAMPLE 2: Fuzzy Matching Completion
// ============================================================================

pub trait FuzzyCompletion {
    fn complete_fuzzy(&self, pattern: &str, candidates: &[String]) -> Vec<(Pair, usize)>;
}

impl FuzzyCompletion for ShellHelper {
    /// Fuzzy match - returns candidates with match scores
    fn complete_fuzzy(&self, pattern: &str, candidates: &[String]) -> Vec<(Pair, usize)> {
        let mut results: Vec<(Pair, usize)> = candidates
            .iter()
            .filter_map(|candidate| {
                let score = fuzzy_score(pattern, candidate);
                if score > 0 {
                    Some((
                        Pair {
                            display: candidate.clone(),
                            replacement: candidate.clone(),
                        },
                        score,
                    ))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (highest first)
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    }
}

/// Simple fuzzy matching score
fn fuzzy_score(pattern: &str, candidate: &str) -> usize {
    let pattern = pattern.to_lowercase();
    let candidate = candidate.to_lowercase();

    // Exact prefix match gets highest score
    if candidate.starts_with(&pattern) {
        return 1000 + pattern.len();
    }

    // Contains substring gets medium score
    if candidate.contains(&pattern) {
        return 500 + pattern.len();
    }

    // Fuzzy character match
    let mut score = 0;
    let mut last_idx = 0;

    for ch in pattern.chars() {
        if let Some(idx) = candidate[last_idx..].find(ch) {
            score += 100;
            last_idx += idx + 1;
        } else {
            return 0; // Character not found
        }
    }

    score
}

// ============================================================================
// EXAMPLE 3: Multi-Column Display
// ============================================================================

pub trait MultiColumnDisplay {
    fn format_columns(&self, items: Vec<String>, term_width: usize) -> String;
}

impl MultiColumnDisplay for ShellHelper {
    fn format_columns(&self, items: Vec<String>, term_width: usize) -> String {
        if items.is_empty() {
            return String::new();
        }

        let max_len = items.iter().map(|s| s.len()).max().unwrap_or(0);
        let col_width = max_len + 2;
        let num_cols = (term_width / col_width).max(1);

        let mut output = String::new();
        for (i, item) in items.iter().enumerate() {
            output.push_str(item);
            output.push_str(&" ".repeat(col_width - item.len()));

            if (i + 1) % num_cols == 0 {
                output.push('\n');
            }
        }

        output
    }
}

// ============================================================================
// EXAMPLE 4: Completion with Descriptions
// ============================================================================

pub struct DescriptivePair {
    pub replacement: String,
    pub display: String,
    pub description: String,
}

pub trait DescriptiveCompletion {
    fn complete_with_descriptions(&self, prefix: &str) -> Vec<DescriptivePair>;
}

impl DescriptiveCompletion for ShellHelper {
    fn complete_with_descriptions(&self, prefix: &str) -> Vec<DescriptivePair> {
        // Command descriptions
        let commands_with_desc = vec![
            ("help", "Show help information"),
            ("actors", "List all actors"),
            ("registry", "Show registered actors"),
            ("info", "Show actor details"),
            ("send", "Send a cast message"),
            ("call", "Send an RPC message"),
            ("stop", "Stop an actor"),
            ("stats", "Show system statistics"),
            ("tree", "Show process group tree"),
            ("cluster", "Show cluster topology"),
        ];

        commands_with_desc
            .into_iter()
            .filter(|(cmd, _)| cmd.starts_with(prefix))
            .map(|(cmd, desc)| DescriptivePair {
                replacement: cmd.to_string(),
                display: format!("{:<15} - {}", cmd, desc),
                description: desc.to_string(),
            })
            .collect()
    }
}

// ============================================================================
// EXAMPLE 5: Contextual Hints
// ============================================================================

pub trait ContextualHints {
    fn get_hint(&self, line: &str, pos: usize) -> Option<String>;
}

impl ContextualHints for ShellHelper {
    fn get_hint(&self, line: &str, pos: usize) -> Option<String> {
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "send" if parts.len() == 1 => {
                Some(" <actor> <json_message>".to_string())
            }
            "call" if parts.len() == 1 => {
                Some(" <actor> <json_message>".to_string())
            }
            "info" if parts.len() == 1 => {
                Some(" <actor_name>".to_string())
            }
            "stop" if parts.len() == 1 => {
                Some(" <actor_name>".to_string())
            }
            "pg" if parts.len() == 1 => {
                Some(" list | members <group>".to_string())
            }
            "cluster" if parts.len() == 1 => {
                Some(" [nodes|groups|actors]".to_string())
            }
            _ => None
        }
    }
}

// ============================================================================
// EXAMPLE 6: Smart Completion with Ranking
// ============================================================================

#[derive(Debug)]
pub struct RankedCandidate {
    pub pair: Pair,
    pub rank: usize,
    pub reason: CompletionReason,
}

#[derive(Debug)]
pub enum CompletionReason {
    ExactPrefix,
    RecentlyUsed,
    FrequentlyUsed,
    Contains,
    Fuzzy,
}

pub trait SmartCompletion {
    fn complete_smart(
        &self,
        pattern: &str,
        candidates: &[String],
        history: &[String],
    ) -> Vec<RankedCandidate>;
}

impl SmartCompletion for ShellHelper {
    fn complete_smart(
        &self,
        pattern: &str,
        candidates: &[String],
        history: &[String],
    ) -> Vec<RankedCandidate> {
        let pattern_lower = pattern.to_lowercase();

        let mut ranked: Vec<RankedCandidate> = candidates
            .iter()
            .filter_map(|candidate| {
                let candidate_lower = candidate.to_lowercase();

                // Exact prefix match - highest priority
                if candidate_lower.starts_with(&pattern_lower) {
                    // Boost if recently used
                    let recent_boost = if history.iter().rev().take(5).any(|h| h == candidate) {
                        100
                    } else {
                        0
                    };

                    return Some(RankedCandidate {
                        pair: Pair {
                            display: candidate.clone(),
                            replacement: candidate.clone(),
                        },
                        rank: 1000 + recent_boost,
                        reason: if recent_boost > 0 {
                            CompletionReason::RecentlyUsed
                        } else {
                            CompletionReason::ExactPrefix
                        },
                    });
                }

                // Contains pattern
                if candidate_lower.contains(&pattern_lower) {
                    return Some(RankedCandidate {
                        pair: Pair {
                            display: candidate.clone(),
                            replacement: candidate.clone(),
                        },
                        rank: 500,
                        reason: CompletionReason::Contains,
                    });
                }

                None
            })
            .collect();

        // Sort by rank (descending)
        ranked.sort_by(|a, b| b.rank.cmp(&a.rank));
        ranked
    }
}

// ============================================================================
// EXAMPLE 7: File Path Completion (for load/send-file commands)
// ============================================================================

pub trait FilePathCompletion {
    fn complete_file_path(&self, partial_path: &str) -> Vec<Pair>;
}

impl FilePathCompletion for ShellHelper {
    fn complete_file_path(&self, partial_path: &str) -> Vec<Pair> {
        use std::path::Path;
        use std::fs;

        let path = Path::new(partial_path);
        let (dir, prefix) = if partial_path.ends_with('/') {
            (path, "")
        } else {
            (
                path.parent().unwrap_or(Path::new(".")),
                path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
            )
        };

        let Ok(entries) = fs::read_dir(dir) else {
            return vec![];
        };

        entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let file_name = entry.file_name().to_str()?.to_string();

                if file_name.starts_with(prefix) {
                    let mut display = file_name.clone();
                    if entry.file_type().ok()?.is_dir() {
                        display.push('/');
                    }

                    Some(Pair {
                        display,
                        replacement: file_name,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_score() {
        assert!(fuzzy_score("demo", "demo_actor_1") > fuzzy_score("demo", "actor_demo"));
        assert!(fuzzy_score("da1", "demo_actor_1") > 0);
        assert_eq!(fuzzy_score("xyz", "demo_actor_1"), 0);
    }

    #[test]
    fn test_case_insensitive() {
        let helper = ShellHelper::new();
        let candidates = vec!["DemoActor".to_string(), "TestActor".to_string()];
        let results = helper.complete_case_insensitive("demo", &candidates);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display, "DemoActor");
    }
}

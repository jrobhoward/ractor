//! Pattern-based filtering for actor traces.

use std::collections::HashSet;

/// Filter for selecting which actors to trace.
#[derive(Debug, Clone)]
pub struct TraceFilter {
    /// Glob patterns to match actor names (e.g., "worker_*", "supervisor")
    patterns: HashSet<String>,
    /// Whether to trace all actors (empty patterns = trace nothing, "*" = trace all)
    trace_all: bool,
}

impl Default for TraceFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceFilter {
    /// Create a new empty filter (traces nothing).
    pub fn new() -> Self {
        Self {
            patterns: HashSet::new(),
            trace_all: false,
        }
    }

    /// Create a filter that traces all actors.
    pub fn all() -> Self {
        Self {
            patterns: HashSet::new(),
            trace_all: true,
        }
    }

    /// Add a pattern to the filter.
    ///
    /// Patterns support glob syntax:
    /// - `*` matches any sequence of characters
    /// - `?` matches any single character
    /// - `worker_*` matches "worker_1", "worker_foo", etc.
    pub fn add_pattern(&mut self, pattern: &str) {
        if pattern == "*" {
            self.trace_all = true;
        } else {
            self.patterns.insert(pattern.to_string());
        }
    }

    /// Remove a pattern from the filter.
    pub fn remove_pattern(&mut self, pattern: &str) -> bool {
        if pattern == "*" {
            self.trace_all = false;
            true
        } else {
            self.patterns.remove(pattern)
        }
    }

    /// Clear all patterns (stop tracing).
    pub fn clear(&mut self) {
        self.patterns.clear();
        self.trace_all = false;
    }

    /// Check if any patterns are active.
    pub fn is_active(&self) -> bool {
        self.trace_all || !self.patterns.is_empty()
    }

    /// Get all active patterns.
    pub fn patterns(&self) -> Vec<&str> {
        if self.trace_all {
            vec!["*"]
        } else {
            self.patterns.iter().map(|s| s.as_str()).collect()
        }
    }

    /// Check if an actor name matches any pattern.
    pub fn matches(&self, actor_name: Option<&str>) -> bool {
        if self.trace_all {
            return true;
        }

        if self.patterns.is_empty() {
            return false;
        }

        let name = match actor_name {
            Some(n) => n,
            None => return false, // Don't trace unnamed actors unless trace_all
        };

        for pattern in &self.patterns {
            if glob_match::glob_match(pattern, name) {
                return true;
            }
        }

        false
    }

    /// Check if an actor ID matches any pattern.
    /// This is a fallback when no name is available.
    pub fn matches_id(&self, actor_id: &str) -> bool {
        if self.trace_all {
            return true;
        }

        for pattern in &self.patterns {
            if glob_match::glob_match(pattern, actor_id) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod filter_tests;

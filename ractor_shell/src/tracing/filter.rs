//! Pattern-based filtering for actor traces.

use std::collections::HashSet;
use std::str::FromStr;

use tracing::Level;

/// Minimum log level for filtering trace events.
/// Events at or above this level will be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinLevel {
    /// Show all events (TRACE and above)
    #[default]
    Trace,
    /// Show DEBUG and above (filter out TRACE)
    Debug,
    /// Show INFO and above (filter out TRACE, DEBUG)
    Info,
    /// Show WARN and above (filter out TRACE, DEBUG, INFO)
    Warn,
    /// Show only ERROR events
    Error,
}

impl MinLevel {
    /// Check if an event at the given level passes this filter.
    pub fn allows(&self, level: Level) -> bool {
        match self {
            MinLevel::Trace => true,
            MinLevel::Debug => level <= Level::DEBUG,
            MinLevel::Info => level <= Level::INFO,
            MinLevel::Warn => level <= Level::WARN,
            MinLevel::Error => level <= Level::ERROR,
        }
    }
}

impl FromStr for MinLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TRACE" => Ok(MinLevel::Trace),
            "DEBUG" => Ok(MinLevel::Debug),
            "INFO" => Ok(MinLevel::Info),
            "WARN" | "WARNING" => Ok(MinLevel::Warn),
            "ERROR" => Ok(MinLevel::Error),
            _ => Err(format!(
                "Invalid log level '{}'. Valid levels: TRACE, DEBUG, INFO, WARN, ERROR",
                s
            )),
        }
    }
}

impl std::fmt::Display for MinLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MinLevel::Trace => write!(f, "TRACE"),
            MinLevel::Debug => write!(f, "DEBUG"),
            MinLevel::Info => write!(f, "INFO"),
            MinLevel::Warn => write!(f, "WARN"),
            MinLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Filter for selecting which actors to trace.
#[derive(Debug, Clone)]
pub struct TraceFilter {
    /// Glob patterns to match actor names (e.g., "worker_*", "supervisor")
    patterns: HashSet<String>,
    /// Whether to trace all actors (empty patterns = trace nothing, "*" = trace all)
    trace_all: bool,
    /// Minimum log level to display (events below this level are filtered out)
    min_level: MinLevel,
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
            min_level: MinLevel::default(),
        }
    }

    /// Create a filter that traces all actors.
    pub fn all() -> Self {
        Self {
            patterns: HashSet::new(),
            trace_all: true,
            min_level: MinLevel::default(),
        }
    }

    /// Set the minimum log level for filtering.
    pub fn set_min_level(&mut self, level: MinLevel) {
        self.min_level = level;
    }

    /// Get the current minimum log level.
    pub fn min_level(&self) -> MinLevel {
        self.min_level
    }

    /// Check if an event at the given level passes the level filter.
    pub fn level_allowed(&self, level: Level) -> bool {
        self.min_level.allows(level)
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
    /// Note: This preserves the min_level setting.
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

    /// Check if a target (module path) matches any pattern.
    /// This allows filtering by module like "ractor_shell::raft*".
    pub fn matches_target(&self, target: &str) -> bool {
        if self.trace_all {
            return true;
        }

        for pattern in &self.patterns {
            if glob_match::glob_match(pattern, target) {
                return true;
            }
        }

        false
    }

    /// Check if any field value matches the filter patterns.
    /// This allows filtering by arbitrary field values like `node = "node_a"`.
    pub fn matches_any_field(&self, fields: &[(String, String)]) -> bool {
        if self.trace_all {
            return true;
        }

        if self.patterns.is_empty() {
            return false;
        }

        for (_key, value) in fields {
            // Strip quotes from the value if present
            let clean_value = value.trim_matches('"');
            for pattern in &self.patterns {
                if glob_match::glob_match(pattern, clean_value) {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod filter_tests;

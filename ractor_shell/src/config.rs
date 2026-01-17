//! Configuration file support for ractor_shell
//!
//! Loads settings from `~/.ractor_shell.toml` if it exists.
//!
//! ## Example Configuration
//!
//! ```toml
//! # ~/.ractor_shell.toml
//!
//! # RPC timeout in seconds (default: 5)
//! rpc_timeout_secs = 10
//!
//! # Default node server port when connecting to remote nodes (default: 9100)
//! node_server_port = 9100
//!
//! # Cluster authentication cookie
//! cluster_cookie = "my_secret_cookie"
//!
//! # Auto-connect to a node on startup
//! # auto_connect = "127.0.0.1:9002"
//!
//! # History file location (default: ~/.ractor_shell_history)
//! # history_file = "/custom/path/history"
//!
//! # Maximum history entries (default: 1000)
//! max_history = 1000
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Shell configuration loaded from file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ShellConfig {
    /// RPC timeout in seconds
    pub rpc_timeout_secs: Option<u64>,

    /// Default node server port
    pub node_server_port: Option<u16>,

    /// Cluster authentication cookie
    pub cluster_cookie: Option<String>,

    /// Auto-connect to this node on startup
    pub auto_connect: Option<String>,

    /// History file location
    pub history_file: Option<String>,

    /// Maximum history entries
    pub max_history: Option<usize>,

    /// Color mode: "auto", "always", or "never"
    pub color: Option<String>,
}

impl ShellConfig {
    /// Load configuration from the default location (~/.ractor_shell.toml)
    pub fn load() -> Self {
        Self::load_from_default_path().unwrap_or_default()
    }

    /// Load configuration from the default path, returning None if not found
    fn load_from_default_path() -> Option<Self> {
        let config_path = Self::default_config_path()?;
        Self::load_from_path(&config_path)
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: &PathBuf) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    /// Get the default configuration file path
    pub fn default_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|mut p| {
            p.push(".ractor_shell.toml");
            p
        })
    }

    /// Get the RPC timeout duration
    pub fn rpc_timeout(&self) -> Duration {
        Duration::from_secs(self.rpc_timeout_secs.unwrap_or(5))
    }

    /// Get the node server port
    pub fn get_node_server_port(&self) -> u16 {
        self.node_server_port
            .unwrap_or(crate::DEFAULT_NODE_SERVER_PORT)
    }

    /// Get the cluster cookie
    pub fn get_cluster_cookie(&self) -> &str {
        self.cluster_cookie
            .as_deref()
            .unwrap_or(crate::DEFAULT_CLUSTER_COOKIE)
    }

    /// Get the history file path
    pub fn history_path(&self) -> Option<PathBuf> {
        if let Some(ref path) = self.history_file {
            Some(PathBuf::from(path))
        } else {
            dirs::home_dir().map(|mut p| {
                p.push(".ractor_shell_history");
                p
            })
        }
    }

    /// Get the maximum history entries
    pub fn get_max_history(&self) -> usize {
        self.max_history.unwrap_or(1000)
    }

    /// Get the color mode setting
    /// Returns None if not set (use CLI default), Some(true) for always, Some(false) for never
    pub fn get_color_enabled(&self) -> Option<bool> {
        match self.color.as_deref() {
            Some("always") => Some(true),
            Some("never") => Some(false),
            _ => None, // "auto" or unset
        }
    }

    /// Create a sample configuration file content
    pub fn sample_config() -> &'static str {
        r#"# Ractor Shell Configuration
# Place this file at ~/.ractor_shell.toml

# RPC timeout in seconds (default: 5)
# rpc_timeout_secs = 5

# Default node server port when connecting to remote nodes (default: 9100)
# node_server_port = 9100

# Cluster authentication cookie (default: "secret_cookie")
# cluster_cookie = "my_secret_cookie"

# Auto-connect to a node on startup
# auto_connect = "127.0.0.1:9002"

# Color output: "auto", "always", or "never" (default: "auto")
# color = "auto"

# History file location (default: ~/.ractor_shell_history)
# history_file = "/custom/path/history"

# Maximum history entries (default: 1000)
# max_history = 1000
"#
    }
}

#[cfg(test)]
mod config_tests;

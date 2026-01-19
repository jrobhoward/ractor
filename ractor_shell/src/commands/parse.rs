//! Command Parsing
//!
//! This module contains the [`ShellCommand`] enum and parsing logic.

use crate::error::{ShellError, ShellResult};

/// Shell commands that can be executed in the REPL.
///
/// Commands are parsed from user input via [`ShellCommand::parse_line`] and
/// executed via [`ShellState::execute`](crate::ShellState::execute).
#[derive(Debug, Clone)]
pub enum ShellCommand {
    /// Display help information. Alias: `help [command]`
    Help { command: Option<String> },
    /// Exit the shell. Aliases: `exit`, `quit`, `q`
    Exit,
    /// List all actors (registered and in process groups). Alias: `a`
    Actors,
    /// List registered actors only. Alias: `r`
    Registry,
    /// List all process groups. Usage: `pg list`
    PgList,
    /// List members of a process group. Usage: `pg members <group>`
    PgMembers { group: String },
    /// Show detailed information about an actor. Alias: `i`
    Info { actor: String },
    /// Send a cast message to an actor. Alias: `s`. Usage: `send <actor> <json>`
    Send { actor: String, message: String },
    /// Send an RPC call to an actor. Alias: `c`. Usage: `call <actor> <json>`
    Call { actor: String, message: String },
    /// Stop an actor gracefully. Usage: `stop <actor>`
    Stop { actor: String },
    /// Connect to a remote node. Usage: `connect <host:port>`
    Connect { host: String },
    /// Disconnect from a remote node. Usage: `disconnect <node>`
    Disconnect { node: String },
    /// Reconnect to a remote node (disconnect + connect). Usage: `reconnect <node>`
    Reconnect { node: String },
    /// List connected nodes
    Nodes,
    /// Switch context to a remote node. Usage: `use <node>` or `use local`
    Use { node: String },
    /// Show cluster topology. Usage: `cluster [nodes|groups|mesh]`
    Cluster { subcommand: Option<String> },
    /// Show system statistics
    Stats,
    /// Show process group tree (limited in Phase 1)
    Tree { actor: Option<String> },
    /// Show actual supervision tree with parent-child relationships. Alias: `st`. Usage: `supervtree [actor]`
    Supervtree { actor: Option<String> },
    /// Show the supervisor (parent) of an actor. Alias: `p`. Usage: `parent <actor>`
    Parent { actor: String },
    /// Send a JSON file as a message. Alias: `sf`. Usage: `send-file <actor> <path>`
    SendFile { actor: String, file_path: String },
    /// Load and execute a script file. Alias: `l`. Usage: `load <path>`
    Load { script_path: String },
    /// Start monitoring an actor's lifecycle. Usage: `monitor <actor>`
    Monitor { actor: String },
    /// Stop monitoring an actor. Usage: `unmonitor <actor>`
    Unmonitor { actor: String },
    /// List all monitored actors and recent events
    Monitors,
    /// Poll and display monitor events (remote only). Usage: `monitor events`
    MonitorEvents,
    /// Launch interactive TUI dashboard. Alias: `t`. Usage: `top`
    Top,
    /// Start tracing actors or list active traces. Usage: `trace [pattern]`
    Trace { pattern: Option<String> },
    /// Stop all tracing. Usage: `trace off`
    TraceOff,
    /// Get or set minimum trace log level. Usage: `trace level [LEVEL]`
    TraceLevel { level: Option<String> },
    /// Log traces to a file. Usage: `trace-to-file <path> [pattern]`
    TraceToFile {
        path: String,
        pattern: Option<String>,
    },
    /// Subscribe to remote traces from a node. Alias: `tr`. Usage: `trace remote <node> <pattern>`
    TraceRemote { node: String, pattern: String },
    /// Stop remote tracing. Usage: `trace remote off`
    TraceRemoteOff,
    /// Show message schema for an actor. Alias: `sc`. Usage: `schema <actor>` or `schema` to list all
    Schema { actor: Option<String> },
    /// Ping a remote node to measure latency. Usage: `ping <node>`
    Ping { node: String },
}

/// Resolve command aliases to their full command names.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(resolve_alias("a"), "actors");
/// assert_eq!(resolve_alias("r"), "registry");
/// assert_eq!(resolve_alias("unknown"), "unknown");
/// ```
pub fn resolve_alias(cmd: &str) -> &str {
    match cmd {
        // Actor inspection
        "a" => "actors",
        "r" => "registry",
        "i" => "info",
        "sc" => "schema",
        "st" => "supervtree",
        "p" => "parent",
        // Messaging
        "s" => "send",
        "c" => "call",
        "sf" => "send-file",
        // Connection & nodes
        "con" => "connect",
        "dis" => "disconnect",
        "n" => "nodes",
        "u" => "use",
        "cl" => "cluster",
        // Monitoring & tracing
        "m" => "monitor",
        "um" => "unmonitor",
        "ms" => "monitors",
        "t" => "top",
        "tr" => "trace",
        "tf" => "trace-to-file",
        // Other
        "h" => "help",
        "l" => "load",
        "q" => "quit",
        _ => cmd,
    }
}

impl ShellCommand {
    /// Resolve command aliases (delegates to module-level function for backward compatibility)
    pub(crate) fn resolve_alias(cmd: &str) -> &str {
        resolve_alias(cmd)
    }

    /// Parse a command line string into a ShellCommand.
    ///
    /// Supports command aliases (e.g., `a` for `actors`, `q` for `quit`).
    /// Returns an error if the command is not recognized or missing required arguments.
    pub fn parse_line(line: &str) -> ShellResult<Self> {
        let mut parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            return Err(ShellError::EmptyCommand);
        }

        // Resolve aliases
        let resolved_cmd = Self::resolve_alias(parts[0]);
        parts[0] = resolved_cmd;

        match parts[0] {
            "help" => Ok(ShellCommand::Help {
                command: parts.get(1).map(|s| s.to_string()),
            }),
            "exit" | "quit" => Ok(ShellCommand::Exit),
            "actors" => Ok(ShellCommand::Actors),
            "registry" => Ok(ShellCommand::Registry),
            "pg" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "pg",
                        requirement: "a subcommand: list, members",
                    });
                }
                match parts[1] {
                    "list" => Ok(ShellCommand::PgList),
                    "members" => {
                        if parts.len() < 3 {
                            return Err(ShellError::MissingArgument {
                                command: "pg members",
                                requirement: "a group name",
                            });
                        }
                        Ok(ShellCommand::PgMembers {
                            group: parts[2].to_string(),
                        })
                    }
                    _ => Err(ShellError::UnknownSubcommand {
                        parent: "pg",
                        subcommand: parts[1].to_string(),
                    }),
                }
            }
            "info" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "info",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Info {
                    actor: parts[1].to_string(),
                })
            }
            "send" => {
                if parts.len() < 3 {
                    return Err(ShellError::MissingArgument {
                        command: "send",
                        requirement: "<actor> <message>",
                    });
                }
                Ok(ShellCommand::Send {
                    actor: parts[1].to_string(),
                    message: parts[2..].join(" "),
                })
            }
            "call" => {
                if parts.len() < 3 {
                    return Err(ShellError::MissingArgument {
                        command: "call",
                        requirement: "<actor> <message>",
                    });
                }
                Ok(ShellCommand::Call {
                    actor: parts[1].to_string(),
                    message: parts[2..].join(" "),
                })
            }
            "stop" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "stop",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Stop {
                    actor: parts[1].to_string(),
                })
            }
            "connect" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "connect",
                        requirement: "<host:port>",
                    });
                }
                Ok(ShellCommand::Connect {
                    host: parts[1].to_string(),
                })
            }
            "disconnect" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "disconnect",
                        requirement: "<node_name>",
                    });
                }
                Ok(ShellCommand::Disconnect {
                    node: parts[1].to_string(),
                })
            }
            "reconnect" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "reconnect",
                        requirement: "<node_name>",
                    });
                }
                Ok(ShellCommand::Reconnect {
                    node: parts[1].to_string(),
                })
            }
            "nodes" => Ok(ShellCommand::Nodes),
            "use" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "use",
                        requirement: "<node_name> (or 'local')",
                    });
                }
                Ok(ShellCommand::Use {
                    node: parts[1].to_string(),
                })
            }
            "cluster" => Ok(ShellCommand::Cluster {
                subcommand: parts.get(1).map(|s| s.to_string()),
            }),
            "stats" => Ok(ShellCommand::Stats),
            "tree" => Ok(ShellCommand::Tree {
                actor: parts.get(1).map(|s| s.to_string()),
            }),
            "supervtree" => Ok(ShellCommand::Supervtree {
                actor: parts.get(1).map(|s| s.to_string()),
            }),
            "parent" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "parent",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Parent {
                    actor: parts[1].to_string(),
                })
            }
            "send-file" | "sendfile" => {
                if parts.len() < 3 {
                    return Err(ShellError::MissingArgument {
                        command: "send-file",
                        requirement: "<actor> <file_path>",
                    });
                }
                Ok(ShellCommand::SendFile {
                    actor: parts[1].to_string(),
                    file_path: parts[2].to_string(),
                })
            }
            "load" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "load",
                        requirement: "<script_path>",
                    });
                }
                Ok(ShellCommand::Load {
                    script_path: parts[1].to_string(),
                })
            }
            "monitor" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "monitor",
                        requirement: "an actor name or 'events'",
                    });
                }
                // Check for subcommands
                if parts[1] == "events" {
                    return Ok(ShellCommand::MonitorEvents);
                }
                Ok(ShellCommand::Monitor {
                    actor: parts[1].to_string(),
                })
            }
            "unmonitor" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "unmonitor",
                        requirement: "an actor name",
                    });
                }
                Ok(ShellCommand::Unmonitor {
                    actor: parts[1].to_string(),
                })
            }
            "monitors" => Ok(ShellCommand::Monitors),
            "top" => Ok(ShellCommand::Top),
            "trace" => {
                match parts.get(1).copied() {
                    Some("off") => Ok(ShellCommand::TraceOff),
                    Some("level") => {
                        // "trace level" or "trace level <LEVEL>"
                        Ok(ShellCommand::TraceLevel {
                            level: parts.get(2).map(|s| s.to_string()),
                        })
                    }
                    Some("remote") => {
                        // "trace remote off" or "trace remote <node> <pattern>"
                        if parts.get(2).copied() == Some("off") {
                            Ok(ShellCommand::TraceRemoteOff)
                        } else if parts.len() < 4 {
                            Err(ShellError::MissingArgument {
                                command: "trace remote",
                                requirement: "<node> <pattern> or 'off'",
                            })
                        } else {
                            Ok(ShellCommand::TraceRemote {
                                node: parts[2].to_string(),
                                pattern: parts[3].to_string(),
                            })
                        }
                    }
                    _ => Ok(ShellCommand::Trace {
                        pattern: parts.get(1).map(|s| s.to_string()),
                    }),
                }
            }
            "trace-to-file" | "tracefile" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "trace-to-file",
                        requirement: "<file_path> [pattern]",
                    });
                }
                Ok(ShellCommand::TraceToFile {
                    path: parts[1].to_string(),
                    pattern: parts.get(2).map(|s| s.to_string()),
                })
            }
            "schema" => Ok(ShellCommand::Schema {
                actor: parts.get(1).map(|s| s.to_string()),
            }),
            "ping" => {
                if parts.len() < 2 {
                    return Err(ShellError::MissingArgument {
                        command: "ping",
                        requirement: "<node> (e.g., 127.0.0.1:9001)",
                    });
                }
                Ok(ShellCommand::Ping {
                    node: parts[1].to_string(),
                })
            }
            _ => Err(ShellError::UnknownCommand(parts[0].to_string())),
        }
    }
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod parse_tests;

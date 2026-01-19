//! Help Command
//!
//! This module contains the help command implementation.

use colored::Colorize;

use crate::{ShellResult, ShellState};

impl ShellState {
    /// Display help information for commands.
    pub(crate) async fn cmd_help(&self, command: Option<String>) -> ShellResult<()> {
        if let Some(cmd) = command {
            // Show detailed help for specific command
            match cmd.as_str() {
                "actors" => {
                    println!("{}", "actors".green().bold());
                    println!("  List all actors in the current context");
                    println!("\n{}", "Usage:".bold());
                    println!("  actors");
                }
                "registry" => {
                    println!("{}", "registry".green().bold());
                    println!("  Show all named/registered actors");
                    println!("\n{}", "Usage:".bold());
                    println!("  registry");
                }
                "pg" => {
                    println!("{}", "pg".green().bold());
                    println!("  Process group commands");
                    println!("\n{}", "Usage:".bold());
                    println!("  pg list              List all process groups");
                    println!("  pg members <group>   Show members of a specific group");
                }
                "info" => {
                    println!("{}", "info <actor>".green().bold());
                    println!("  Show detailed information about an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  info <actor_name>");
                }
                "send" => {
                    println!("{}", "send <actor> <message>".green().bold());
                    println!("  Send a message to an actor (fire-and-forget)");
                    println!("\n{}", "Usage:".bold());
                    println!("  send <actor_name> <json_message>");
                }
                "call" => {
                    println!("{}", "call <actor> <message>".green().bold());
                    println!("  Send a message and wait for reply");
                    println!("\n{}", "Usage:".bold());
                    println!("  call <actor_name> <json_message>");
                }
                "stop" => {
                    println!("{}", "stop <actor>".green().bold());
                    println!("  Stop an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  stop <actor_name>");
                }
                "send-file" | "sendfile" => {
                    println!("{}", "send-file <actor> <file>".green().bold());
                    println!("  Send a message from a JSON file to an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  send-file <actor_name> <file_path>");
                    println!("\n{}", "Example:".bold());
                    println!("  send-file ping_pong messages/ping.json");
                }
                "load" => {
                    println!("{}", "load <script>".green().bold());
                    println!("  Execute shell commands from a script file");
                    println!("\n{}", "Usage:".bold());
                    println!("  load <script_path>");
                    println!("\n{}", "Example:".bold());
                    println!("  load scripts/setup.txt");
                    println!("\n{}", "Script Format:".bold());
                    println!("  - One command per line");
                    println!("  - Lines starting with # are comments");
                    println!("  - Empty lines are ignored");
                }
                "monitor" => {
                    println!("{}", "monitor <actor>".green().bold());
                    println!("  Start monitoring an actor's lifecycle events");
                    println!("\n{}", "Usage:".bold());
                    println!("  monitor <actor_name>   Monitor a specific actor");
                    println!(
                        "  monitor events         Poll and display monitor events (remote only)"
                    );
                    println!("\n{}", "Example:".bold());
                    println!("  monitor demo_actor_1");
                    println!("  monitor events");
                    println!("\n{}", "Note:".bold());
                    println!("  - Shows start, stop, panic, and kill events");
                    println!("  - Local: events are displayed automatically in real-time");
                    println!("  - Remote: use 'monitor events' to poll for events");
                }
                "unmonitor" => {
                    println!("{}", "unmonitor <actor>".green().bold());
                    println!("  Stop monitoring an actor");
                    println!("\n{}", "Usage:".bold());
                    println!("  unmonitor <actor_name>");
                    println!("\n{}", "Example:".bold());
                    println!("  unmonitor demo_actor_1");
                }
                "monitors" => {
                    println!("{}", "monitors".green().bold());
                    println!("  List all currently monitored actors");
                    println!("\n{}", "Usage:".bold());
                    println!("  monitors");
                }
                "top" => {
                    println!("{}", "top".green().bold());
                    println!("  Launch interactive TUI dashboard for actor monitoring");
                    println!("\n{}", "Usage:".bold());
                    println!("  top");
                    println!("\n{}", "Alias:".bold());
                    println!("  t");
                    println!("\n{}", "Keyboard Shortcuts:".bold());
                    println!("  q, Esc      Quit dashboard");
                    println!("  ↑/k, ↓/j    Navigate up/down");
                    println!("  s           Cycle sort column");
                    println!("  S           Toggle sort direction");
                    println!("  /           Enter filter mode");
                    println!("  c           Clear filter");
                    println!("  r           Force refresh");
                    println!("  ?           Show help");
                }
                "trace" => {
                    println!("{}", "trace [pattern]".green().bold());
                    println!("  Trace actor message flow and lifecycle events");
                    println!("\n{}", "Usage:".bold());
                    println!("  trace              List active traces and current settings");
                    println!("  trace <pattern>    Start tracing actors matching pattern");
                    println!("  trace off          Stop all tracing");
                    println!("  trace level        Show current minimum log level");
                    println!(
                        "  trace level <LVL>  Set minimum level (TRACE|DEBUG|INFO|WARN|ERROR)"
                    );
                    println!("\n{}", "Alias:".bold());
                    println!("  tr");
                    println!("\n{}", "Patterns:".bold());
                    println!("  worker_*     Match actors starting with 'worker_'");
                    println!("  *            Match all actors");
                    println!("  supervisor   Match exact name 'supervisor'");
                    println!("\n{}", "Log Levels (most to least verbose):".bold());
                    println!("  TRACE        Show all events (default)");
                    println!("  DEBUG        Filter out TRACE events");
                    println!("  INFO         Filter out TRACE and DEBUG events");
                    println!("  WARN         Show only WARN and ERROR events");
                    println!("  ERROR        Show only ERROR events");
                    println!("\n{}", "Examples:".bold());
                    println!("  trace worker_*     Start tracing worker actors");
                    println!("  trace *            Trace all actors");
                    println!("  trace level INFO   Show only INFO, WARN, ERROR events");
                    println!("  trace off          Stop tracing");
                    println!("\n{}", "Note:".bold());
                    println!("  Traces show actor span enter/exit and tracing events.");
                    println!("  Requires actors to use ractor's tracing instrumentation.");
                }
                "trace-to-file" | "tracefile" => {
                    println!("{}", "trace-to-file <path> [pattern]".green().bold());
                    println!("  Log actor traces to a file");
                    println!("\n{}", "Usage:".bold());
                    println!("  trace-to-file <path>           Log all traces to file");
                    println!("  trace-to-file <path> <pattern> Log matching traces to file");
                    println!("\n{}", "Alias:".bold());
                    println!("  tf");
                    println!("\n{}", "Examples:".bold());
                    println!("  trace-to-file /tmp/trace.log worker_*");
                    println!("  trace-to-file ./debug.log");
                    println!("\n{}", "Note:".bold());
                    println!("  File output is in addition to console output.");
                    println!("  Use 'trace off' to stop and close file outputs.");
                }
                "trace-remote" | "traceremote" => {
                    println!("{}", "trace remote <node> <pattern>".green().bold());
                    println!("  Subscribe to trace events from a remote node");
                    println!("\n{}", "Usage:".bold());
                    println!("  trace remote <node> <pattern>  Subscribe to remote traces");
                    println!("  trace remote off               Stop all remote subscriptions");
                    println!("\n{}", "Examples:".bold());
                    println!("  trace remote 127.0.0.1:9001 raft_*");
                    println!("  trace remote 127.0.0.1:9001 *");
                    println!("  trace remote off");
                    println!("\n{}", "Note:".bold());
                    println!("  Remote traces are displayed with [node] prefix.");
                    println!("  Events are polled periodically from the remote node.");
                    println!("  If buffer overflows, dropped count is reported.");
                }
                "ping" => {
                    println!("{}", "ping <node>".green().bold());
                    println!("  Ping a remote node to measure round-trip latency");
                    println!("\n{}", "Usage:".bold());
                    println!("  ping <host:port>");
                    println!("\n{}", "Examples:".bold());
                    println!("  ping 127.0.0.1:9001");
                    println!("\n{}", "Note:".bold());
                    println!("  Auto-connects if not already connected to the node.");
                    println!("  Returns latency in milliseconds.");
                }
                _ => {
                    println!("{} Unknown command: {}", "Error:".red().bold(), cmd);
                }
            }
        } else {
            // Show general help
            println!("{}", "Available Commands:".bold().underline());
            println!();
            println!("{}", "  Local Introspection:".bright_black());
            println!("  {}  Show this help message", "help [command]".green());
            println!("  {}            List all actors", "actors".green());
            println!("  {}          Show registered actors", "registry".green());
            println!("  {}            List process groups", "pg list".green());
            println!(
                "  {}    Show process group members",
                "pg members <group>".green()
            );
            println!("  {}       Show actor details", "info <actor>".green());
            println!("  {} Send a cast message", "send <actor> <msg>".green());
            println!("  {} Send an RPC message", "call <actor> <msg>".green());
            println!("  {}       Stop an actor", "stop <actor>".green());
            println!();
            println!("{}", "  Message & Script Input:".bright_black());
            println!(
                "  {} Send message from file",
                "send-file <actor> <file>".green()
            );
            println!("  {} Execute shell script", "load <script>".green());
            println!();
            println!("{}", "  Remote Connection:".bright_black());
            println!(
                "  {}  Connect to remote node",
                "connect <host:port>".green()
            );
            println!("  {} Disconnect from node", "disconnect <node>".green());
            println!("  {}             List connected nodes", "nodes".green());
            println!("  {}       Switch to a node context", "use <node>".green());
            println!(
                "  {}       Ping node and measure latency",
                "ping <node>".green()
            );
            println!();
            println!("{}", "  Cluster Topology:".bright_black());
            println!("  {}          Show cluster topology", "cluster".green());
            println!("  {}       Show cluster nodes", "cluster nodes".green());
            println!("  {}      Show process groups", "cluster groups".green());
            println!("  {}      Show all actors", "cluster actors".green());
            println!();
            println!("{}", "  System Introspection:".bright_black());
            println!("  {}             Show system statistics", "stats".green());
            println!("  {}       Show process group tree", "tree".green());
            println!(
                "  {}               Launch TUI actor dashboard",
                "top".green()
            );
            println!();
            println!("{}", "  Actor Monitoring:".bright_black());
            println!(
                "  {}    Start monitoring actor events",
                "monitor <actor>".green()
            );
            println!(
                "  {}  Stop monitoring an actor",
                "unmonitor <actor>".green()
            );
            println!("  {}          List monitored actors", "monitors".green());
            println!();
            println!("{}", "  Tracing:".bright_black());
            println!(
                "  {}   Trace actors (pattern or 'off')",
                "trace [pattern]".green()
            );
            println!("  {}  Log traces to file", "trace-to-file <path>".green());
            println!(
                "  {} Subscribe to remote traces",
                "trace remote <node> <pattern>".green()
            );
            println!();
            println!("  {}              Exit the shell", "exit".green());
            println!();
            println!("{}", "  Shell Features:".bright_black());
            println!(
                "  {} Use TAB to auto-complete commands, actors, and groups",
                "•".bright_cyan()
            );
            println!(
                "  {} Command history with UP/DOWN arrows",
                "•".bright_cyan()
            );
            println!(
                "  {} Aliases: {}, {}, {}, {}, {}, {}, {}",
                "•".bright_cyan(),
                "a=actors".bright_black(),
                "r=registry".bright_black(),
                "i=info".bright_black(),
                "s=send".bright_black(),
                "c=call".bright_black(),
                "t=top".bright_black(),
                "q=quit".bright_black()
            );
            println!();
            println!("Type 'help <command>' for more information on a specific command");
        }
        Ok(())
    }
}

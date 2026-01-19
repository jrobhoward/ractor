//! Supervision Commands
//!
//! Commands for viewing actor supervision hierarchy:
//! - `tree` - Show process group tree
//! - `supervtree` - Show actual supervision tree
//! - `parent` - Show supervisor (parent) of an actor

use std::collections::HashSet;

use colored::Colorize;
use ractor::rpc::CallResult;
use ractor::{pg, registry, ActorCell};

use crate::error::{ShellError, ShellResult};
use crate::protocol::{self, ShellProtocolMessage};
use crate::ShellState;
use crate::{DEFAULT_RPC_TIMEOUT, KNOWN_PROCESS_GROUPS};

impl ShellState {
    /// Show process group tree.
    pub(crate) async fn cmd_tree(&self, _actor: Option<String>) -> ShellResult<()> {
        println!();
        println!("{}", "Process Group Tree".bold().underline());
        println!();

        // Helper function to display the tree
        fn display_tree(tree: &std::collections::HashMap<String, Vec<crate::protocol::ActorInfo>>) {
            if tree.is_empty() {
                println!("No process groups found");
                return;
            }

            let group_names: Vec<_> = tree.keys().collect();
            for (group_idx, group_name) in group_names.iter().enumerate() {
                let is_last_group = group_idx == group_names.len() - 1;
                let group_prefix = if is_last_group {
                    "└──"
                } else {
                    "├──"
                };
                let members = tree.get(*group_name).unwrap();

                println!(
                    "{} {} {}",
                    group_prefix.bright_cyan(),
                    group_name.green().bold(),
                    format!("[{} members]", members.len()).bright_black()
                );

                for (i, actor) in members.iter().enumerate() {
                    let is_last = i == members.len() - 1;
                    let prefix = if is_last { "└──" } else { "├──" };
                    let connector = if is_last_group { " " } else { "│" };
                    let actor_name = actor.name.clone().unwrap_or_else(|| "-".to_string());
                    let local_marker = if actor.is_local { "" } else { " (remote)" };
                    println!(
                        "{}   {} {} {}{}",
                        connector.bright_cyan(),
                        prefix.bright_cyan(),
                        actor_name.cyan(),
                        format!("({})", actor.id).bright_black(),
                        local_marker.bright_black()
                    );
                }
                println!();
            }
        }

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::GetProcessGroupTree,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(tree) => {
                        println!(
                            "{}",
                            format!("Process groups on {} ({} total):", node_name, tree.len())
                                .bright_black()
                        );
                        println!();
                        display_tree(&tree);
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
                return Ok(());
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution - use pg::which_groups() for dynamic group discovery
        let groups = pg::which_groups();
        let mut tree: std::collections::HashMap<String, Vec<crate::protocol::ActorInfo>> =
            std::collections::HashMap::new();

        for group in groups {
            let members = pg::get_members(&group);
            let actors: Vec<crate::protocol::ActorInfo> = members
                .iter()
                .map(crate::protocol::ActorInfo::from_cell)
                .collect();
            tree.insert(group, actors);
        }

        println!(
            "{}",
            format!("Process groups ({} total):", tree.len()).bright_black()
        );
        println!();
        display_tree(&tree);

        println!(
            "{}",
            "Tip: Use 'supervtree' to see actor supervision hierarchy".bright_black()
        );
        println!();

        Ok(())
    }

    /// Show actual supervision tree with parent-child relationships.
    pub(crate) async fn cmd_supervtree(&self, actor: Option<String>) -> ShellResult<()> {
        println!();
        println!("{}", "Supervision Tree".bold().underline());
        println!();

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetSupervisionTree(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(tree_nodes) => {
                        if tree_nodes.is_empty() {
                            if let Some(ref name) = actor {
                                return Err(ShellError::RemoteActorNotFound {
                                    actor: name.clone(),
                                    node: node_name.clone(),
                                });
                            } else {
                                println!(
                                    "{}",
                                    "No root actors found on remote node.".bright_black()
                                );
                            }
                        } else {
                            for (i, tree_node) in tree_nodes.iter().enumerate() {
                                let is_last = i == tree_nodes.len() - 1;
                                self.print_supervision_subtree_remote(tree_node, "", is_last);
                                if !is_last {
                                    println!();
                                }
                            }
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            if let Some(actor_name) = actor {
                // Show tree starting from specific actor
                if let Some(cell) = registry::where_is(actor_name.clone()) {
                    self.print_supervision_subtree(&cell, "", true);
                } else {
                    return Err(ShellError::ActorNotFound(actor_name));
                }
            } else {
                // Find and display all root actors (those without supervisors)
                let all_actors = self.get_all_local_actors();
                let roots: Vec<ActorCell> = all_actors
                    .iter()
                    .filter(|cell| cell.try_get_supervisor().is_none())
                    .cloned()
                    .collect();

                if roots.is_empty() {
                    println!("{}", "No root actors found.".bright_black());
                } else {
                    for (i, root) in roots.iter().enumerate() {
                        let is_last = i == roots.len() - 1;
                        self.print_supervision_subtree(root, "", is_last);
                        if !is_last {
                            println!();
                        }
                    }
                }
            }
        }

        println!();
        Ok(())
    }

    /// Show the supervisor (parent) of an actor.
    pub(crate) async fn cmd_parent(&self, actor: String) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetActorParent(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(parent_info) => {
                        println!("{}", format!("Actor on {}: {}", node_name, actor).bold());
                        match parent_info {
                            Some(supervisor) => {
                                let supervisor_name =
                                    supervisor.name.unwrap_or_else(|| supervisor.id.to_string());
                                println!(
                                    "  Supervisor: {} {}",
                                    supervisor_name.green(),
                                    format!("({})", supervisor.id).bright_black()
                                );
                                println!("  Status:     {}", supervisor.status);
                            }
                            None => {
                                println!(
                                    "  {}",
                                    "No supervisor (this is a root actor)".bright_black()
                                );
                            }
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            if let Some(cell) = registry::where_is(actor.clone()) {
                match cell.try_get_supervisor() {
                    Some(supervisor) => {
                        println!("{}", format!("Actor: {}", actor).bold());
                        println!(
                            "  Supervisor: {} {}",
                            supervisor
                                .get_name()
                                .unwrap_or_else(|| "<unnamed>".to_string())
                                .green(),
                            format!("({})", supervisor.get_id()).bright_black()
                        );
                        println!("  Status:     {:?}", supervisor.get_status());
                    }
                    None => {
                        println!("{}", format!("Actor: {}", actor).bold());
                        println!(
                            "  {}",
                            "No supervisor (this is a root actor)".bright_black()
                        );
                    }
                }
            } else {
                return Err(ShellError::ActorNotFound(actor));
            }
        }
        Ok(())
    }

    /// Helper function to get all local actors from registry and process groups.
    pub(crate) fn get_all_local_actors(&self) -> Vec<ActorCell> {
        let mut actors = Vec::new();
        let mut seen_ids = HashSet::new();

        // Add registered actors
        for name in registry::registered() {
            if let Some(cell) = registry::where_is(name) {
                let id = cell.get_id();
                if seen_ids.insert(id) {
                    actors.push(cell);
                }
            }
        }

        // Add actors from known process groups
        for group in KNOWN_PROCESS_GROUPS {
            for cell in pg::get_members(&group.to_string()) {
                let id = cell.get_id();
                if seen_ids.insert(id) {
                    actors.push(cell);
                }
            }
        }

        actors
    }

    /// Recursively print a supervision tree starting from the given actor.
    pub(crate) fn print_supervision_subtree(&self, cell: &ActorCell, prefix: &str, is_last: bool) {
        let actor_name = cell.get_name().unwrap_or_else(|| "<unnamed>".to_string());
        let actor_id = cell.get_id().to_string();
        let status = format!("{:?}", cell.get_status());
        let children = cell.get_children();
        let child_count = children.len();

        // Print current actor
        let branch = if is_last { "└──" } else { "├──" };
        let name_part = if child_count > 0 {
            format!("{} [{}]", actor_name, child_count).green().bold()
        } else {
            actor_name.cyan()
        };

        println!(
            "{}{} {} {} {}",
            prefix.bright_cyan(),
            branch.bright_cyan(),
            name_part,
            format!("({})", actor_id).bright_black(),
            format!("[{}]", status).bright_black()
        );

        // Print children recursively
        if !children.is_empty() {
            let child_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };

            for (i, child) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;
                self.print_supervision_subtree(child, &child_prefix, is_last_child);
            }
        }
    }

    /// Recursively print a supervision tree from remote SupervisionTreeNode data.
    pub(crate) fn print_supervision_subtree_remote(
        &self,
        node: &protocol::SupervisionTreeNode,
        prefix: &str,
        is_last: bool,
    ) {
        let actor_name = node
            .actor
            .name
            .clone()
            .unwrap_or_else(|| "<unnamed>".to_string());
        let actor_id = &node.actor.id;
        let status = &node.actor.status;
        let child_count = node.child_count;

        // Print current actor
        let branch = if is_last { "└──" } else { "├──" };
        let name_part = if child_count > 0 {
            format!("{} [{}]", actor_name, child_count).green().bold()
        } else {
            actor_name.cyan()
        };

        println!(
            "{}{} {} {} {}",
            prefix.bright_cyan(),
            branch.bright_cyan(),
            name_part,
            format!("({})", actor_id).bright_black(),
            format!("[{}]", status).bright_black()
        );

        // Print children recursively
        if !node.children.is_empty() {
            let child_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };

            for (i, child) in node.children.iter().enumerate() {
                let is_last_child = i == node.children.len() - 1;
                self.print_supervision_subtree_remote(child, &child_prefix, is_last_child);
            }
        }
    }
}

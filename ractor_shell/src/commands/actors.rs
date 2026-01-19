//! Actor Discovery Commands
//!
//! Commands for discovering and inspecting actors:
//! - `actors` - List all actors
//! - `registry` - List registered actors
//! - `pg list` - List process groups
//! - `pg members` - List group members
//! - `info` - Show actor details
//! - `schema` - Show message schema

use colored::Colorize;
use ractor::rpc::CallResult;
use ractor::{pg, registry};
use tabled::Tabled;

use crate::error::{ShellError, ShellResult};
use crate::protocol::ShellProtocolMessage;
use crate::ShellState;
use crate::{schema_registry, table, DEFAULT_RPC_TIMEOUT};

impl ShellState {
    /// List all actors in the current context.
    pub(crate) async fn cmd_actors(&self) -> ShellResult<()> {
        #[derive(Tabled)]
        struct ActorRow {
            #[tabled(rename = "Name")]
            name: String,
            #[tabled(rename = "ID")]
            id: String,
            #[tabled(rename = "Status")]
            status: String,
        }

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListRegisteredActors,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        if actors.is_empty() {
                            println!("No registered actors on {}.", node_name);
                            return Ok(());
                        }

                        let mut rows = Vec::new();
                        for actor_info in actors {
                            rows.push(ActorRow {
                                name: actor_info.name.unwrap_or_else(|| "-".to_string()),
                                id: actor_info.id,
                                status: actor_info.status,
                            });
                        }

                        println!("{}", table::build_table(rows));
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            println!("{}", "Listing actors...".bright_black());
            println!("Note: Phase 1 only shows named actors. Full enumeration coming in Phase 2.");

            let registered = registry::registered();

            if registered.is_empty() {
                println!("No registered actors found.");
                return Ok(());
            }

            let mut rows = Vec::new();
            for name in registered {
                if let Some(cell) = registry::where_is(name.clone()) {
                    rows.push(ActorRow {
                        name: name.clone(),
                        id: cell.get_id().to_string(),
                        status: format!("{:?}", cell.get_status()),
                    });
                }
            }

            println!("{}", table::build_table(rows));
        }

        Ok(())
    }

    /// List registered actors only.
    pub(crate) async fn cmd_registry(&self) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListRegisteredActors,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        if actors.is_empty() {
                            println!("No registered actors on {}.", node_name);
                            return Ok(());
                        }

                        println!(
                            "{}",
                            format!("Registered Actors on {} ({}):", node_name, actors.len())
                                .bold()
                        );
                        for actor_info in actors {
                            println!(
                                "  {} {} {}",
                                actor_info.name.unwrap_or_else(|| "-".to_string()).green(),
                                "→".bright_black(),
                                actor_info.id.bright_black()
                            );
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local registry query
            let registered = registry::registered();

            if registered.is_empty() {
                println!("No registered actors.");
                return Ok(());
            }

            println!(
                "{}",
                format!("Registered Actors ({}):", registered.len()).bold()
            );
            for name in registered {
                if let Some(cell) = registry::where_is(name.clone()) {
                    println!(
                        "  {} {} {}",
                        name.green(),
                        "→".bright_black(),
                        cell.get_id().to_string().bright_black()
                    );
                }
            }
        }

        Ok(())
    }

    /// List all process groups.
    pub(crate) async fn cmd_pg_list(&self) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        ShellProtocolMessage::ListProcessGroups,
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(groups) => {
                        if groups.is_empty() {
                            println!("No process groups found on {}", node_name);
                            return Ok(());
                        }

                        println!(
                            "{}",
                            format!("Process groups on {} ({} total):", node_name, groups.len())
                                .bold()
                        );
                        for group in groups {
                            println!("  - {}", group.green());
                        }
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
                return Ok(());
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        }

        // Local execution
        let groups = pg::which_groups();
        if groups.is_empty() {
            println!("No process groups found");
            println!();
            println!(
                "  {} Use 'pg members <group>' to query specific groups",
                "•".bright_black()
            );
            return Ok(());
        }

        println!(
            "{}",
            format!("Process groups ({} total):", groups.len()).bold()
        );
        for group in groups {
            println!("  - {}", group.green());
        }

        Ok(())
    }

    /// List members of a process group.
    pub(crate) async fn cmd_pg_members(&self, group: String) -> ShellResult<()> {
        #[derive(Tabled)]
        struct MemberRow {
            #[tabled(rename = "Actor ID")]
            id: String,
            #[tabled(rename = "Name")]
            name: String,
            #[tabled(rename = "Local")]
            is_local: String,
        }

        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetProcessGroupMembers(group.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(actors) => {
                        if actors.is_empty() {
                            println!(
                                "Process group '{}' has no members on {} (or doesn't exist)",
                                group, node_name
                            );
                            return Ok(());
                        }

                        println!(
                            "{}",
                            format!(
                                "Members of '{}' on {} ({}):",
                                group,
                                node_name,
                                actors.len()
                            )
                            .bold()
                        );

                        let mut rows = Vec::new();
                        for actor_info in actors {
                            rows.push(MemberRow {
                                id: actor_info.id,
                                name: actor_info.name.unwrap_or_else(|| "-".to_string()),
                                is_local: if actor_info.is_local { "yes" } else { "no" }
                                    .to_string(),
                            });
                        }

                        let table = table::build_table(rows);
                        println!("{}", table);
                    }
                    CallResult::Timeout => return Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => return Err(ShellError::RpcSenderError),
                }
            } else {
                return Err(ShellError::NodeNotConnected(node_name.clone()));
            }
        } else {
            // Local query
            let members = pg::get_members(&group);

            if members.is_empty() {
                println!(
                    "Process group '{}' has no members (or doesn't exist)",
                    group
                );
                return Ok(());
            }

            println!(
                "{}",
                format!("Members of '{}' ({}):", group, members.len()).bold()
            );

            let mut rows = Vec::new();
            for cell in members {
                let id = cell.get_id();
                let name = cell.get_name().unwrap_or_else(|| "-".to_string());
                let is_local = if id.is_local() { "yes" } else { "no" };

                rows.push(MemberRow {
                    id: id.to_string(),
                    name,
                    is_local: is_local.to_string(),
                });
            }

            let table = table::build_table(rows);
            println!("{}", table);
        }

        Ok(())
    }

    /// Show detailed information about an actor.
    pub(crate) async fn cmd_info(&self, actor: String) -> ShellResult<()> {
        // Check if we're working with a remote node
        if let Some(node_name) = &self.current_node {
            if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                let result = introspection_ref
                    .call(
                        |reply| ShellProtocolMessage::GetActorInfo(actor.clone(), reply),
                        Some(DEFAULT_RPC_TIMEOUT),
                    )
                    .await
                    .map_err(ShellError::messaging)?;

                match result {
                    CallResult::Success(Some(actor_info)) => {
                        println!("{}", format!("Actor on {}: {}", node_name, actor).bold());
                        println!("  ID:     {}", actor_info.id);
                        println!("  Status: {}", actor_info.status);
                        if let Some(name) = actor_info.name {
                            println!("  Name:   {}", name);
                        }
                        println!(
                            "  Local:  {}",
                            if actor_info.is_local { "yes" } else { "no" }
                        );
                        Ok(())
                    }
                    CallResult::Success(None) => Err(ShellError::RemoteActorNotFound {
                        actor: actor.clone(),
                        node: node_name.clone(),
                    }),
                    CallResult::Timeout => Err(ShellError::rpc_timeout()),
                    CallResult::SenderError => Err(ShellError::RpcSenderError),
                }
            } else {
                Err(ShellError::NodeNotConnected(node_name.clone()))
            }
        } else {
            // Local query
            if let Some(cell) = registry::where_is(actor.clone()) {
                println!("{}", format!("Actor: {}", actor).bold());
                println!("  ID:     {}", cell.get_id());
                println!("  Status: {:?}", cell.get_status());
                if let Some(name) = cell.get_name() {
                    println!("  Name:   {}", name);
                }

                // Show supervision information
                match cell.try_get_supervisor() {
                    Some(supervisor) => {
                        let supervisor_name = supervisor
                            .get_name()
                            .unwrap_or_else(|| format!("{}", supervisor.get_id()));
                        println!("  Supervisor: {}", supervisor_name.green());
                    }
                    None => {
                        println!("  Supervisor: {}", "None (root actor)".bright_black());
                    }
                }

                let children = cell.get_children();
                println!("  Children: {}", children.len());
                if !children.is_empty() {
                    for child in children.iter() {
                        let child_name = child
                            .get_name()
                            .unwrap_or_else(|| format!("{}", child.get_id()));
                        println!("    - {}", child_name.cyan());
                    }
                }

                Ok(())
            } else {
                Err(ShellError::ActorNotFound(actor))
            }
        }
    }

    /// Show message schema for an actor, or list all schema-enabled actors.
    ///
    /// # Arguments
    ///
    /// * `actor` - Optional actor name. If None, lists all schema-enabled actors.
    /// * `show_all` - If false, only show RPC variants. If true, show all variants.
    pub(crate) async fn cmd_schema(
        &self,
        actor: Option<String>,
        show_all: bool,
    ) -> ShellResult<()> {
        match actor {
            Some(actor_name) => {
                // Show schema for a specific actor
                if let Some(node_name) = &self.current_node {
                    // Remote query
                    if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                        let result = introspection_ref
                            .call(
                                |reply| {
                                    ShellProtocolMessage::GetMessageSchema(
                                        actor_name.clone(),
                                        reply,
                                    )
                                },
                                Some(DEFAULT_RPC_TIMEOUT),
                            )
                            .await
                            .map_err(ShellError::messaging)?;

                        match result {
                            CallResult::Success(Some(schema_json)) => {
                                self.display_schema(&actor_name, &schema_json, show_all);
                                Ok(())
                            }
                            CallResult::Success(None) => {
                                println!(
                                    "{} Actor '{}' does not have a registered schema.",
                                    "ℹ".cyan(),
                                    actor_name
                                );
                                println!(
                                    "{}",
                                    "  Hint: The actor may use DynamicMessage or not support schema introspection."
                                        .bright_black()
                                );
                                Ok(())
                            }
                            CallResult::Timeout => Err(ShellError::rpc_timeout()),
                            CallResult::SenderError => Err(ShellError::RpcSenderError),
                        }
                    } else {
                        Err(ShellError::NodeNotConnected(node_name.clone()))
                    }
                } else {
                    // Local query
                    if let Some(schema_json) = schema_registry::get_schema(&actor_name) {
                        self.display_schema(&actor_name, &schema_json, show_all);
                    } else {
                        println!(
                            "{} Actor '{}' does not have a registered schema.",
                            "ℹ".cyan(),
                            actor_name
                        );
                        println!(
                            "{}",
                            "  Hint: The actor may use DynamicMessage or not have registered a schema."
                                .bright_black()
                        );
                    }
                    Ok(())
                }
            }
            None => {
                // List all schema-enabled actors
                if let Some(node_name) = &self.current_node {
                    // Remote query
                    if let Some(introspection_ref) = self.connected_nodes.get(node_name) {
                        let result = introspection_ref
                            .call(
                                ShellProtocolMessage::ListSchemaActors,
                                Some(DEFAULT_RPC_TIMEOUT),
                            )
                            .await
                            .map_err(ShellError::messaging)?;

                        match result {
                            CallResult::Success(schema_actors) => {
                                if schema_actors.is_empty() {
                                    println!(
                                        "{}",
                                        "No schema-enabled actors on this node.".bright_black()
                                    );
                                } else {
                                    println!(
                                        "{} on {}:",
                                        "Schema-enabled actors".bold(),
                                        node_name.cyan()
                                    );
                                    for actor in schema_actors {
                                        println!("  {}", actor.name.green());
                                    }
                                    println!();
                                    println!(
                                        "{}",
                                        "Use 'schema <actor>' to see the message schema."
                                            .bright_black()
                                    );
                                }
                                Ok(())
                            }
                            CallResult::Timeout => Err(ShellError::rpc_timeout()),
                            CallResult::SenderError => Err(ShellError::RpcSenderError),
                        }
                    } else {
                        Err(ShellError::NodeNotConnected(node_name.clone()))
                    }
                } else {
                    // Local query
                    let schemas = schema_registry::list_schemas();
                    if schemas.is_empty() {
                        println!("{}", "No schema-enabled actors registered.".bright_black());
                        println!(
                            "{}",
                            "  Hint: Actors need to call schema_registry::register() in pre_start."
                                .bright_black()
                        );
                    } else {
                        println!("{}", "Schema-enabled actors:".bold());
                        for (name, _) in &schemas {
                            println!("  {}", name.green());
                        }
                        println!();
                        println!(
                            "{}",
                            "Use 'schema <actor>' to see the message schema.".bright_black()
                        );
                    }
                    Ok(())
                }
            }
        }
    }

    /// Display a formatted schema for an actor.
    ///
    /// # Arguments
    ///
    /// * `actor_name` - The name of the actor
    /// * `schema_json` - The JSON schema string
    /// * `show_all` - If false, only show RPC variants. If true, show all variants.
    pub(crate) fn display_schema(&self, actor_name: &str, schema_json: &str, show_all: bool) {
        println!(
            "{} {}",
            "Message schema for".bold(),
            actor_name.green().bold()
        );
        println!();

        // Parse and display the schema
        if let Ok(schema) = serde_json::from_str::<serde_json::Value>(schema_json) {
            let formatted = schema_registry::format_schema(&schema, show_all);
            print!("{}", formatted);

            // Show usage example - find the first RPC variant
            if let Some(variants) = schema.get("variants").and_then(|v| v.as_object()) {
                // Find an RPC variant to show as example
                let rpc_variant = variants
                    .iter()
                    .find(|(_, info)| info.get("rpc").and_then(|v| v.as_bool()).unwrap_or(false));

                if let Some((variant_name, _)) = rpc_variant {
                    println!("{}", "Example usage:".bright_black());
                    println!(
                        "  {}",
                        format!(
                            "call {} {} {{\"field\": \"value\"}}",
                            actor_name, variant_name
                        )
                        .cyan()
                    );
                }
            }
        } else {
            // Fallback: just print the raw JSON
            println!("{}", schema_json);
        }
    }
}

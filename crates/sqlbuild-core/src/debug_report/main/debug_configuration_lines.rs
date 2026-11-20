//! The configuration section of `sqb debug`.

use crate::debug_report::constants::{NOT_CONFIGURED, NOT_PRESENT};
use crate::debug_report::models::{DebugLine, DebugProjectFacts, DebugStatus, WarehouseCheck};
use crate::process_environment::models::EnvFileMarker;

fn resolved(label: &str, message: &str, status_message: &str) -> DebugLine {
    DebugLine::checked(label, message, (DebugStatus::Ok, Some(status_message)))
}

/// Which env file loaded, by path and variable count; never a value.
fn env_file_line(marker: &EnvFileMarker) -> DebugLine {
    let Some(path) = &marker.path else {
        return DebugLine::checked(
            "env file",
            "off",
            (DebugStatus::Skip, Some("env_file = \"\"")),
        );
    };
    if !marker.loaded {
        return DebugLine::checked(
            "env file",
            path,
            (DebugStatus::Ok, Some("not found, optional")),
        );
    }
    let count: usize = marker.names.len();
    let mut status: String = format!(
        "loaded, {count} variable{}",
        if count == 1 { "" } else { "s" }
    );
    if !marker.overridden.is_empty() {
        status.push_str(&format!(
            ", {} already set in the environment",
            marker.overridden.len()
        ));
    }
    DebugLine::checked("env file", path, (DebugStatus::Ok, Some(&status)))
}

fn warehouse_line(check: &WarehouseCheck) -> DebugLine {
    match check {
        WarehouseCheck::NotUsed {
            group,
            adapter_name,
            source,
        } => DebugLine::checked(
            &format!("{group} warehouse"),
            &format!("not used (adapter {adapter_name})"),
            (DebugStatus::Skip, Some(source)),
        ),
        WarehouseCheck::Unresolved { group, error } => DebugLine::checked(
            &format!("{group} warehouse"),
            &format!("error: {error}"),
            (DebugStatus::Error, Some("unresolved")),
        ),
        WarehouseCheck::Resolved {
            group,
            warehouse,
            source,
        } => resolved(
            &format!("{group} warehouse"),
            warehouse.as_deref().unwrap_or("not set"),
            source,
        ),
    }
}

/// Project file, local config, project, adapter, target, namespace and warehouse lines.
#[must_use]
pub fn debug_configuration_lines(
    facts: &DebugProjectFacts,
    warehouses: &[WarehouseCheck],
) -> Vec<DebugLine> {
    let local_file: Option<&str> = facts.local_file.as_deref();
    let mut lines: Vec<DebugLine> = vec![
        resolved("project file", &facts.project_file, "found and valid"),
        resolved(
            "local config",
            local_file.unwrap_or(NOT_PRESENT),
            if local_file.is_some() {
                "found"
            } else {
                NOT_PRESENT
            },
        ),
    ];
    lines.extend(facts.env_file.as_ref().map(env_file_line));
    lines.extend([
        resolved("project", &facts.project_name, "loaded"),
        resolved("adapter", &facts.adapter_name, "found"),
        resolved(
            "target",
            facts.target_name.as_deref().unwrap_or("default"),
            "resolved",
        ),
        resolved(
            "connection",
            facts.connection_name.as_deref().unwrap_or("inline"),
            "resolved",
        ),
        resolved(
            "database",
            facts.database.as_deref().unwrap_or(NOT_CONFIGURED),
            "resolved",
        ),
        resolved(
            "schema",
            facts.schema.as_deref().unwrap_or(NOT_CONFIGURED),
            "resolved",
        ),
    ]);
    lines.extend(warehouses.iter().map(warehouse_line));
    lines
}

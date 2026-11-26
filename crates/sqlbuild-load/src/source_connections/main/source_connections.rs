//! Read source connections into SQLLoad's connection types, with SQLLoad's own checks and messages.

use std::collections::BTreeMap;

use sqlload::project::errors::ProjectError;
use sqlload::project::main::parse_project::parse_project;
use sqlload::project::models::ConnectionConfig;

use crate::source_connections::_helpers::literal::literal;
use crate::source_connections::constants::CONNECTIONS;

/// Each of `connections` as SQLLoad's connection of its `kind`, every string taken literally, or SQLLoad's message.
pub fn source_connections(
    connections: toml::Table,
) -> Result<BTreeMap<String, ConnectionConfig>, String> {
    let mut project = toml::Table::new();
    project.insert(
        CONNECTIONS.to_owned(),
        literal(toml::Value::Table(connections)),
    );
    let text: String = toml::to_string(&project).map_err(|error| error.to_string())?;
    match parse_project(&text, &|_| None) {
        Ok(project) => Ok(project.connections),
        Err(ProjectError::Syntax(message)) => Err(message),
        Err(error) => Err(error.to_string()),
    }
}

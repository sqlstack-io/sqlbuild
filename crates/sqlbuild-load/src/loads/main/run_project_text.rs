//! The SQLLoad project a load runs as: its connection, a target writing through SQLBuild's session, and the load.

use crate::literal_settings::main::literal_settings::literal_settings;
use crate::loads::constants::{CONNECTIONS, SOURCES};
use crate::loads::models::LoadTarget;

/// The project text that loads `settings` as the source `load` into `target`, reading `connections`; every string is taken literally.
pub fn run_project_text(
    (load, settings): (&str, toml::Table),
    connections: toml::Table,
    target: &LoadTarget,
) -> Result<String, String> {
    let mut target_settings = toml::Table::new();
    target_settings.insert("connection".to_owned(), target.session.clone().into());
    if let Some(schema) = &target.loader_schema {
        target_settings.insert("loader_schema".to_owned(), schema.clone().into());
    }
    let mut targets = toml::Table::new();
    targets.insert(target.name.clone(), toml::Value::Table(target_settings));
    let mut sources = toml::Table::new();
    sources.insert(
        load.to_owned(),
        literal_settings(toml::Value::Table(settings)),
    );
    let mut project = toml::Table::new();
    project.insert("default_target".to_owned(), target.name.clone().into());
    project.insert(
        CONNECTIONS.to_owned(),
        literal_settings(toml::Value::Table(connections)),
    );
    project.insert("targets".to_owned(), toml::Value::Table(targets));
    project.insert(SOURCES.to_owned(), toml::Value::Table(sources));
    toml::to_string(&project).map_err(|error| error.to_string())
}

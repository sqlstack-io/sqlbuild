//! The SQLLoad project a load is checked as: the source connections, a target that never connects, and the load as a source.

use std::collections::BTreeMap;
use std::sync::Arc;

use sqlload::adapters::session::errors::SessionError;
use sqlload::adapters::session::models::SessionConnection;
use sqlload::adapters::session::types::{CallerConnection, CallerSession};

use crate::literal_settings::main::literal_settings::literal_settings;
use crate::loads::constants::{CHECK_CONNECTION, CHECK_SCHEMA, CHECK_TARGET, CONNECTIONS, SOURCES};
use crate::loads::models::LoadCheck;

/// The warehouse a compile check writes to, which it never opens.
#[derive(Debug)]
pub(crate) struct UnopenedWarehouse;

impl CallerConnection for UnopenedWarehouse {
    fn open(&self) -> Result<Arc<dyn CallerSession>, SessionError> {
        Err(SessionError::new(
            "a compile check never connects to the warehouse",
        ))
    }

    fn describe(&self) -> String {
        "the project's warehouse".to_owned()
    }
}

/// The project text holding `source`, settings as the source of that name when given, and the session its target writes through.
pub(crate) fn check_project(
    check: &LoadCheck,
    source: Option<(&str, toml::Table)>,
) -> Result<(String, BTreeMap<String, SessionConnection>), String> {
    let mut target = toml::Table::new();
    target.insert("connection".to_owned(), CHECK_CONNECTION.into());
    target.insert("loader_schema".to_owned(), CHECK_SCHEMA.into());
    let mut targets = toml::Table::new();
    targets.insert(CHECK_TARGET.to_owned(), toml::Value::Table(target));
    let mut sources = toml::Table::new();
    if let Some((load, settings)) = source {
        sources.insert(
            load.to_owned(),
            literal_settings(toml::Value::Table(settings)),
        );
    }
    let mut project = toml::Table::new();
    project.insert("default_target".to_owned(), CHECK_TARGET.into());
    project.insert(
        CONNECTIONS.to_owned(),
        literal_settings(toml::Value::Table(check.connections.clone())),
    );
    project.insert("targets".to_owned(), toml::Value::Table(targets));
    if !sources.is_empty() {
        project.insert(SOURCES.to_owned(), toml::Value::Table(sources));
    }
    let sessions = BTreeMap::from([(
        CHECK_CONNECTION.to_owned(),
        SessionConnection {
            warehouse: check.warehouse,
            motherduck: check.motherduck,
            database: None,
            caller: Arc::new(UnopenedWarehouse),
        },
    )]);
    Ok((
        toml::to_string(&project).map_err(|error| error.to_string())?,
        sessions,
    ))
}

//! The checks `check_loader` runs on a `@loader`'s options before and after SQLLoad reads them.

use sqlload::python_sources::models::PythonSource;

use crate::loads::_helpers::settings::refuse_warehouse_name;
use crate::loads::constants::{
    COLUMNS, CONNECTION, LOADER_OWNED_KEYS, ON_SCHEMA_CHANGE, RENAMED_STRATEGY, REPLACE_STRATEGY,
    TABLE_NAME, TABLES, WRITE_STRATEGY,
};
use crate::loads::models::{LoadCheck, LoadProblem, LoaderDeclaration};

/// An option SQLBuild decides itself, or SQLBuild's old name for the replace strategy.
pub(crate) fn refused_option(loader: &LoaderDeclaration) -> Result<(), LoadProblem> {
    for (key, instead) in LOADER_OWNED_KEYS {
        if loader.settings.contains_key(key) {
            return Err(LoadProblem {
                message: format!("@loader can't set {key}; {instead}"),
                path: vec![key.to_owned()],
            });
        }
    }
    if loader.fills_source && loader.settings.contains_key(ON_SCHEMA_CHANGE) {
        return Err(LoadProblem {
            message: format!(
                "@loader can't set {ON_SCHEMA_CHANGE}; set it on the source it fills, in its \
                 sources file"
            ),
            path: vec![ON_SCHEMA_CHANGE.to_owned()],
        });
    }
    if loader
        .settings
        .get(WRITE_STRATEGY)
        .and_then(toml::Value::as_str)
        == Some(RENAMED_STRATEGY)
    {
        return Err(LoadProblem {
            message: format!(
                "write_strategy \"{RENAMED_STRATEGY}\" is now \"{REPLACE_STRATEGY}\", as SQLLoad names it"
            ),
            path: vec![WRITE_STRATEGY.to_owned()],
        });
    }
    Ok(())
}

/// A connection the loader names that is a target's warehouse, not a source connection.
pub(crate) fn refused_connection(
    check: &LoadCheck,
    loader: &LoaderDeclaration,
) -> Result<(), LoadProblem> {
    refuse_warehouse_name(check, loader.connection.as_deref())
}

/// The loader's options with the declared column types of the source it fills.
pub(crate) fn table_settings(loader: &LoaderDeclaration) -> toml::Table {
    let mut table: toml::Table = loader.settings.clone();
    if !loader.columns.is_empty() {
        let mut columns = toml::Table::new();
        for (column, kind) in &loader.columns {
            columns.insert(column.clone(), toml::Value::String(kind.clone()));
        }
        table.insert(COLUMNS.to_owned(), toml::Value::Table(columns));
    }
    if let Some(policy) = &loader.on_schema_change {
        table.insert(
            ON_SCHEMA_CHANGE.to_owned(),
            toml::Value::String(policy.clone()),
        );
    }
    table
}

/// `loader` as the one-table Python source SQLLoad declares for an `@sl.table`.
pub(crate) fn python_source(
    loader: &LoaderDeclaration,
    table: &toml::Table,
) -> Result<PythonSource, LoadProblem> {
    let mut settings = serde_json::Map::new();
    if let Some(connection) = &loader.connection {
        settings.insert(CONNECTION.to_owned(), connection.clone().into());
    }
    settings.insert(TABLE_NAME.to_owned(), loader.table.clone().into());
    let mut tables = serde_json::Map::new();
    tables.insert(
        loader.name.clone(),
        serde_json::to_value(table).map_err(|error| LoadProblem {
            message: error.to_string(),
            path: Vec::new(),
        })?,
    );
    settings.insert(TABLES.to_owned(), serde_json::Value::Object(tables));
    Ok(PythonSource::new(
        loader.name.clone(),
        loader.module.clone(),
        settings,
    ))
}

/// SQLLoad's message about the Python source it knows `loader` as, named as the `@loader`.
pub(crate) fn loader_problem(loader: &LoaderDeclaration, message: &str) -> LoadProblem {
    let mut text: String = message.trim_end().to_owned();
    for kind in ["table", "source"] {
        let place: String = format!("Python {kind} {} ({})", loader.name, loader.module);
        text = text
            .replace(&format!("{place}: "), "")
            .replace(&format!(" in {place}"), "")
            .replace(&place, &format!("@loader '{}'", loader.name));
    }
    LoadProblem {
        message: text,
        path: Vec::new(),
    }
}

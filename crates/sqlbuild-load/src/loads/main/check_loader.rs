//! Check a `@loader`'s options as SQLLoad reads a Python table's, without connecting anywhere.

use sqlload::project::errors::ProjectError;
use sqlload::project::main::parse_python_project::parse_python_project;
use sqlload::python_sources::models::PythonSource;

use crate::loads::_helpers::project::check_project;
use crate::loads::constants::{
    COLUMNS, CONNECTION, LOADER_OWNED_KEYS, RENAMED_STRATEGY, REPLACE_STRATEGY, TABLE_NAME, TABLES,
    WRITE_STRATEGY,
};
use crate::loads::models::{LoadCheck, LoadProblem, LoaderDeclaration};

/// `loader`'s table settings as SQLLoad reads them, with the columns SQLBuild gives it, or why they can't be used.
pub fn check_loader(
    check: &LoadCheck,
    loader: &LoaderDeclaration,
) -> Result<toml::Table, LoadProblem> {
    for (key, instead) in LOADER_OWNED_KEYS {
        if loader.settings.contains_key(key) {
            return Err(LoadProblem {
                message: format!("@loader can't set {key}; {instead}"),
                path: vec![key.to_owned()],
            });
        }
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
    if let Some(connection) = &loader.connection
        && let Some((_, sets_kind)) = check
            .warehouse_connections
            .iter()
            .find(|(name, _)| name == connection)
    {
        let why: &str = if *sets_kind {
            "is a target's warehouse connection"
        } else {
            "sets no kind"
        };
        return Err(LoadProblem {
            message: format!(
                "connection '{connection}' {why}; loads and loaders read source connections"
            ),
            path: vec![CONNECTION.to_owned()],
        });
    }
    let mut table: toml::Table = loader.settings.clone();
    if !loader.columns.is_empty() {
        let mut columns = toml::Table::new();
        for (column, kind) in &loader.columns {
            columns.insert(column.clone(), toml::Value::String(kind.clone()));
        }
        table.insert(COLUMNS.to_owned(), toml::Value::Table(columns));
    }
    let source: PythonSource = python_source(loader, &table)?;
    let (text, sessions) = check_project(check, None).map_err(|message| LoadProblem {
        message,
        path: Vec::new(),
    })?;
    match parse_python_project(&text, &|_| None, (&[source], sessions)) {
        Ok(_) => Ok(table),
        Err(ProjectError::Syntax(message)) => Err(loader_problem(loader, &message)),
        Err(ProjectError::Invalid(problems)) => Err(loader_problem(loader, &problems.join("\n"))),
        Err(error) => Err(loader_problem(loader, &error.to_string())),
    }
}

/// `loader` as the one-table Python source SQLLoad declares for an `@sl.table`.
fn python_source(
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
    Ok(PythonSource {
        name: loader.name.clone(),
        module: loader.module.clone(),
        settings,
        http_paths: std::collections::BTreeMap::new(),
    })
}

/// SQLLoad's message about the Python source it knows `loader` as, named as the `@loader`.
fn loader_problem(loader: &LoaderDeclaration, message: &str) -> LoadProblem {
    LoadProblem {
        message: message.trim_end().replace(
            &format!("Python source {} ({})", loader.name, loader.module),
            &format!("@loader '{}'", loader.name),
        ),
        path: Vec::new(),
    }
}

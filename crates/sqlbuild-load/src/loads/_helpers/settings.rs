//! A load's settings before SQLLoad reads them: the connection it reads, and its sources' column types on its tables.

use crate::loads::_helpers::naming::literal_tables;
use crate::loads::constants::{COLUMNS, CONNECTION, INCLUDE, TABLES};
use crate::loads::models::{FilledSource, LoadCheck, LoadDeclaration, LoadProblem};

/// A problem at `path` in the `load:` mapping.
pub(crate) fn at(path: &[&str], message: String) -> LoadProblem {
    LoadProblem {
        message,
        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
    }
}

/// Refuse a load that reads one of the project's SQLBuild connections.
pub(crate) fn refuse_warehouse_connection(
    check: &LoadCheck,
    load: &LoadDeclaration,
) -> Result<(), LoadProblem> {
    let Some(name) = load.settings.get(CONNECTION).and_then(toml::Value::as_str) else {
        return Ok(());
    };
    let Some((_, sets_kind)) = check
        .warehouse_connections
        .iter()
        .find(|(warehouse, _)| warehouse == name)
    else {
        return Ok(());
    };
    let why: &str = match sets_kind {
        true => "is a warehouse connection",
        false => "sets no kind",
    };
    Err(at(
        &[CONNECTION],
        format!("connection '{name}' {why}; loads and loaders read source connections"),
    ))
}

/// `load`'s settings with each listed source's column types on the table it fills, where compile knows that table.
pub(crate) fn with_source_columns(load: &LoadDeclaration) -> Result<toml::Table, LoadProblem> {
    let mut settings: toml::Table = load.settings.clone();
    let mut tables: toml::Table = match settings.remove(TABLES) {
        None => toml::Table::new(),
        Some(toml::Value::Table(tables)) => tables,
        Some(_) => {
            return Err(at(
                &[TABLES],
                "the load's tables must be a mapping by `schema.table`".to_owned(),
            ));
        }
    };
    if let Some((key, _)) = tables
        .iter()
        .find(|(_, table)| table.get(COLUMNS).is_some())
    {
        return Err(at(
            &[TABLES, key, COLUMNS],
            "a load's tables can't set columns; declare each column's type on the source the \
             table loads into, under `sources:`"
                .to_owned(),
        ));
    }
    if let Some(known) = literal_tables(&load.name, &load.settings) {
        for source in &load.sources {
            tables = with_columns(tables, (&load.name, &known), source)?;
        }
    }
    if !tables.is_empty() {
        settings.insert(TABLES.to_owned(), toml::Value::Table(tables));
    }
    Ok(settings)
}

/// `tables` with `source`'s column types pinned on the included table it loads from, or refusing a source no included table loads into.
fn with_columns(
    mut tables: toml::Table,
    (load, known): (&str, &[(String, String)]),
    source: &FilledSource,
) -> Result<toml::Table, LoadProblem> {
    let Some((key, _)) = known
        .iter()
        .find(|(_, destination)| *destination == source.name)
    else {
        let loads_into: Vec<&str> = known
            .iter()
            .map(|(_, destination)| destination.as_str())
            .collect();
        return Err(at(
            &[INCLUDE],
            format!(
                "source '{}' matches no table the load '{load}' includes; its tables load into {}",
                source.name,
                loads_into.join(", ")
            ),
        ));
    };
    if source.columns.is_empty() {
        return Ok(tables);
    }
    let mut columns = toml::Table::new();
    for (column, kind) in &source.columns {
        columns.insert(column.clone(), toml::Value::String(kind.clone()));
    }
    let table = tables
        .entry(key.clone())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if let Some(table) = table.as_table_mut() {
        table.insert(COLUMNS.to_owned(), toml::Value::Table(columns));
    }
    Ok(tables)
}

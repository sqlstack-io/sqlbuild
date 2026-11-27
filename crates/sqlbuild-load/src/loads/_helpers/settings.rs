//! A load's settings before SQLLoad reads them: the connection it reads, and its sources' column types on its tables.

use sqlload::column_types::models::Warehouse;

use crate::loads::_helpers::naming::literal_tables;
use crate::loads::constants::{
    COLUMNS, CONNECTION, FILES_KIND, INCLUDE, KIND, ON_SCHEMA_CHANGE, TABLE_NAME, TABLES,
    TEMPLATE_OPEN,
};
use crate::loads::main::sqlload_column_type::sqlload_column_type;
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
    refuse_warehouse_name(
        check,
        load.settings.get(CONNECTION).and_then(toml::Value::as_str),
    )
}

/// Refuse `name` when it is one of the project's SQLBuild connections; loads and loaders read source connections.
pub(crate) fn refuse_warehouse_name(
    check: &LoadCheck,
    name: Option<&str>,
) -> Result<(), LoadProblem> {
    let Some(name) = name else {
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
        true => "is a target's warehouse connection",
        false => "sets no kind",
    };
    Err(at(
        &[CONNECTION],
        format!("connection '{name}' {why}; loads and loaders read source connections"),
    ))
}

/// `load`'s settings with each listed source's column types on the table it fills, where compile knows that table.
pub(crate) fn with_source_columns(
    load: &LoadDeclaration,
    warehouse: Warehouse,
) -> Result<toml::Table, LoadProblem> {
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
            tables = with_columns(tables, (&load.name, &known), (source, warehouse))?;
        }
    } else if let Some(known) = one_named_table(load) {
        let filling = load
            .sources
            .iter()
            .filter(|source| known.iter().any(|(_, table)| *table == source.name));
        for source in filling {
            tables = with_columns(tables, (&load.name, &known), (source, warehouse))?;
        }
    }
    if !tables.is_empty() {
        settings.insert(TABLES.to_owned(), toml::Value::Table(tables));
    }
    Ok(settings)
}

/// `tables` with `source`'s column types and `on_schema_change` on the included table it loads from, or refusing a source no included table loads into.
fn with_columns(
    mut tables: toml::Table,
    (load, known): (&str, &[(String, String)]),
    (source, warehouse): (&FilledSource, Warehouse),
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
    let table = tables
        .entry(key.clone())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(table) = table.as_table_mut() else {
        return Ok(tables);
    };
    if let Some(policy) = &source.on_schema_change {
        if table.contains_key(ON_SCHEMA_CHANGE) {
            return Err(at(
                &[TABLES, key, ON_SCHEMA_CHANGE],
                format!(
                    "source '{}' sets on_schema_change, so the load's table can't; keep it on \
                     the source",
                    source.name
                ),
            ));
        }
        table.insert(
            ON_SCHEMA_CHANGE.to_owned(),
            toml::Value::String(policy.clone()),
        );
    }
    if !source.columns.is_empty() {
        table.insert(
            COLUMNS.to_owned(),
            toml::Value::Table(column_types(&source.columns, warehouse)),
        );
    }
    if table.is_empty() {
        tables.remove(key);
    }
    Ok(tables)
}

/// The one table of a files load naming it with a literal `table_name`, keyed by the source as SQLLoad keys it, which is the load; a source it doesn't fill is S304's to report.
fn one_named_table(load: &LoadDeclaration) -> Option<Vec<(String, String)>> {
    if load.settings.get(KIND)?.as_str()? != FILES_KIND {
        return None;
    }
    let name: &str = load.settings.get(TABLE_NAME)?.as_str()?;
    (!name.contains(TEMPLATE_OPEN)).then(|| vec![(load.name.clone(), name.to_owned())])
}

/// Each column's declared type as SQLLoad reads it in `warehouse`.
pub(crate) fn column_types(columns: &[(String, String)], warehouse: Warehouse) -> toml::Table {
    columns
        .iter()
        .map(|(column, kind)| {
            (
                column.clone(),
                toml::Value::String(sqlload_column_type(kind, warehouse)),
            )
        })
        .collect()
}

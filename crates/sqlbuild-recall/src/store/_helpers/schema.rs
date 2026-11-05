//! Schema state checks and additive column upgrades of a SQLite store.

use std::collections::HashSet;

use rusqlite::Connection;

use crate::schema::constants::{ALL_TABLES, RECALL_SCHEMA_VERSION, VIEW_NAMES};
use crate::schema::main::add_column_statement::add_column_statement;
use crate::schema::main::table_columns::table_columns;
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::{ColumnSpec, DdlOptions, Dialect, Table};
use crate::store::_helpers::sqlite::{checked, stored_schema_version};
use crate::store::errors::StoreError;

pub(crate) fn sqlite_options() -> DdlOptions {
    DdlOptions {
        database: None,
        retention_days: 0,
        cluster: None,
        producer_version: String::new(),
    }
}

/// Whether every table, column and view exists at this build's schema version, read without a lock.
pub(crate) fn current(connection: &Connection) -> Result<bool, StoreError> {
    if stored_schema_version(connection)?.is_none_or(|version| version < RECALL_SCHEMA_VERSION) {
        return Ok(false);
    }
    let present: i64 = checked(
        connection.query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type IN ('table', 'view') \
             AND name IN (SELECT value FROM json_each(?1))",
            [expected_names()],
            |row| row.get(0),
        ),
        "inspect Recall store",
    )?;
    if present != i64::try_from(ALL_TABLES.len() + VIEW_NAMES.len()).unwrap_or(i64::MAX) {
        return Ok(false);
    }
    Ok(missing_columns(connection)?.is_empty())
}

fn expected_names() -> String {
    let names: Vec<&str> = ALL_TABLES
        .iter()
        .map(|table| table_spec(*table).name)
        .chain(VIEW_NAMES)
        .collect();
    serde_json::Value::from(names).to_string()
}

/// The names of the columns `table` has in the store.
pub(crate) fn stored_columns(
    connection: &Connection,
    table: Table,
) -> Result<HashSet<String>, StoreError> {
    let mut statement = checked(
        connection.prepare_cached(&format!(
            "SELECT name FROM pragma_table_xinfo('{}')",
            table_spec(table).name
        )),
        "inspect Recall table",
    )?;
    checked(
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .and_then(Iterator::collect),
        "inspect Recall table",
    )
}

/// Every column this build writes that the stored tables lack.
fn missing_columns(connection: &Connection) -> Result<Vec<(Table, ColumnSpec)>, StoreError> {
    let mut missing: Vec<(Table, ColumnSpec)> = Vec::new();
    for table in ALL_TABLES {
        let existing: HashSet<String> = stored_columns(connection, table)?;
        missing.extend(
            table_columns(table)
                .into_iter()
                .filter(|column| !existing.contains(column.name))
                .map(|column| (table, column)),
        );
    }
    Ok(missing)
}

pub(crate) fn add_missing_columns(connection: &Connection) -> Result<(), StoreError> {
    let options = sqlite_options();
    for (table, column) in missing_columns(connection)? {
        checked(
            connection.execute_batch(&add_column_statement(
                Dialect::Sqlite,
                &options,
                (table_spec(table), &column),
            )),
            "add Recall column",
        )?;
    }
    Ok(())
}

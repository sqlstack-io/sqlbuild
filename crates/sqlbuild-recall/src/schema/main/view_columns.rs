//! `view_columns`: the columns each view of this build exposes.

use std::collections::{BTreeMap, BTreeSet};

use crate::schema::constants::VIEW_NAMES;
use crate::schema::main::ddl_script::ddl_script;
use crate::schema::models::{DdlOptions, Dialect};

/// Each view's column names, from a scratch in-memory store built with this build's DDL, or the SQLite error text.
pub fn view_columns() -> Result<BTreeMap<&'static str, BTreeSet<String>>, String> {
    let connection = rusqlite::Connection::open_in_memory().map_err(|error| error.to_string())?;
    let options = DdlOptions {
        database: None,
        retention_days: 1,
        cluster: None,
        producer_version: String::new(),
    };
    connection
        .execute_batch(&ddl_script(Dialect::Sqlite, &options))
        .map_err(|error| error.to_string())?;
    let mut columns: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
    let mut statement = connection
        .prepare("SELECT name FROM pragma_table_info(?1)")
        .map_err(|error| error.to_string())?;
    for view in VIEW_NAMES {
        let names = statement
            .query_map([view], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<BTreeSet<String>, _>>()
            .map_err(|error| error.to_string())?;
        columns.insert(view, names);
    }
    Ok(columns)
}

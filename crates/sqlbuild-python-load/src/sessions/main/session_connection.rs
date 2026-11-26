//! A SQLBuild target's warehouse connection as the SQLLoad connection its loads write through.

use std::sync::Arc;

use pyo3::prelude::{Py, PyAny};
use sqlload::adapters::session::models::SessionConnection;
use sqlload::column_types::models::Warehouse;

use crate::sessions::models::PythonConnection;

/// The session connection for `adapter`'s warehouse, opening each session with `opener`; adapters SQLLoad can't load into fail naming the adapter.
pub fn session_connection(
    adapter: &str,
    database: Option<String>,
    (opener, description): (Py<PyAny>, String),
) -> Result<SessionConnection, String> {
    let (warehouse, motherduck): (Warehouse, bool) = match adapter {
        "duckdb" => (Warehouse::DuckDb, false),
        "motherduck" => (Warehouse::DuckDb, true),
        "snowflake" => (Warehouse::Snowflake, false),
        "databricks" => (Warehouse::Databricks, false),
        "bigquery" => (Warehouse::BigQuery, false),
        "postgres" => (Warehouse::Postgres, false),
        "clickhouse" => (Warehouse::ClickHouse, false),
        other => {
            return Err(format!(
                "loads and loaders can't write to {other} yet; SQLLoad loads into duckdb, \
                 motherduck, snowflake, databricks, bigquery, postgres and clickhouse"
            ));
        }
    };
    Ok(SessionConnection {
        warehouse,
        motherduck,
        database,
        caller: Arc::new(PythonConnection {
            opener,
            description,
        }),
    })
}

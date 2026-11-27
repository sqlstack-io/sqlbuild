//! The SQLLoad warehouse a SQLBuild adapter's targets load into.

use sqlload::column_types::models::Warehouse;

/// `adapter`'s warehouse and whether it is MotherDuck's, or the error naming an adapter SQLLoad can't load into.
pub fn load_warehouse(adapter: &str) -> Result<(Warehouse, bool), String> {
    Ok(match adapter {
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
    })
}

//! The SQL dialect SQLBuild reads a SQLLoad warehouse's types in.

use sqlload::column_types::models::Warehouse;

/// `warehouse`'s SQLBuild dialect name, such as `duckdb` for DuckDB and MotherDuck.
#[must_use]
pub fn warehouse_dialect(warehouse: Warehouse) -> &'static str {
    match warehouse {
        Warehouse::Snowflake => "snowflake",
        Warehouse::DuckDb | Warehouse::Parquet => "duckdb",
        Warehouse::Databricks => "databricks",
        Warehouse::BigQuery => "bigquery",
        Warehouse::Postgres => "postgres",
        Warehouse::ClickHouse => "clickhouse",
    }
}

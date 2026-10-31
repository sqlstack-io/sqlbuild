//! `table_statements`: idempotent `CREATE TABLE` and index statements of every Recall table.

use crate::schema::_helpers::relational_tables::relational_table;
use crate::schema::_helpers::rendering::{
    all_columns, clickhouse_engine, clickhouse_identifier, clickhouse_table, clickhouse_ttl,
    clickhouse_type, on_cluster,
};
use crate::schema::constants::ALL_TABLES;
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::{DdlOptions, Dialect, TableSpec};

fn clickhouse_table_statement(spec: &TableSpec, options: &DdlOptions) -> String {
    let columns: Vec<String> = all_columns(spec)
        .iter()
        .map(|column| {
            format!(
                "    {} {}",
                clickhouse_identifier(column.name),
                clickhouse_type(column)
            )
        })
        .collect();
    format!(
        "CREATE TABLE IF NOT EXISTS {name}{cluster} (\n{columns}\n)\nENGINE = {engine}\n\
         PARTITION BY toDate({expiry})\nORDER BY ({order})\nTTL {ttl}\n\
         SETTINGS ttl_only_drop_parts = 1, non_replicated_deduplication_window = 1000",
        name = clickhouse_table(spec.name, options),
        cluster = on_cluster(options),
        columns = columns.join(",\n"),
        engine = clickhouse_engine(spec, options),
        expiry = spec.expiry_column,
        order = spec.clickhouse_order_by.join(", "),
        ttl = clickhouse_ttl(spec, options),
    )
}

/// Every table's creation statements, in dependency order; safe to rerun.
#[must_use]
pub fn table_statements(dialect: Dialect, options: &DdlOptions) -> Vec<String> {
    ALL_TABLES
        .iter()
        .flat_map(|table| {
            let spec: &TableSpec = table_spec(*table);
            match dialect {
                Dialect::Sqlite => relational_table(spec, false, false),
                Dialect::ClickHouse => vec![clickhouse_table_statement(spec, options)],
            }
        })
        .collect()
}

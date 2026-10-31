//! Column, table and identifier rendering shared by the DDL statements.

use crate::schema::constants::COMMON_COLUMNS;
use crate::schema::models::{ColumnSpec, ColumnType, DdlOptions, TableSpec};

/// The table's own columns followed by the common columns.
pub(crate) fn all_columns(spec: &TableSpec) -> Vec<ColumnSpec> {
    spec.columns
        .iter()
        .chain(COMMON_COLUMNS.iter())
        .copied()
        .collect()
}

pub(crate) fn sqlite_type(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::Text | ColumnType::Timestamp | ColumnType::Json => "TEXT",
        ColumnType::Integer | ColumnType::Boolean => "INTEGER",
        ColumnType::Real => "REAL",
        ColumnType::Blob => "BLOB",
    }
}

pub(crate) fn clickhouse_type(column: &ColumnSpec) -> String {
    let base: &str = match column.column_type {
        ColumnType::Text | ColumnType::Json | ColumnType::Blob => "String",
        ColumnType::Integer => "Int64",
        ColumnType::Real => "Float64",
        ColumnType::Boolean => "UInt8",
        ColumnType::Timestamp => "DateTime64(6, 'UTC')",
    };
    if column.nullable {
        format!("Nullable({base})")
    } else {
        base.to_owned()
    }
}

/// `` `name` `` for ClickHouse.
pub(crate) fn clickhouse_identifier(name: &str) -> String {
    format!("`{}`", name.replace('\\', "\\\\").replace('`', "\\`"))
}

/// A ClickHouse table or view name, qualified by the configured database.
pub(crate) fn clickhouse_table(name: &str, options: &DdlOptions) -> String {
    match &options.database {
        Some(database) => format!(
            "{}.{}",
            clickhouse_identifier(database),
            clickhouse_identifier(name)
        ),
        None => clickhouse_identifier(name),
    }
}

/// ` ON CLUSTER `c`` when a cluster is configured.
pub(crate) fn on_cluster(options: &DdlOptions) -> String {
    options
        .cluster
        .as_deref()
        .map(|cluster| format!(" ON CLUSTER {}", clickhouse_identifier(cluster)))
        .unwrap_or_default()
}

/// The TTL expression of a table: expiry column plus retention, artifacts outliving references.
pub(crate) fn clickhouse_ttl(spec: &TableSpec, options: &DdlOptions) -> String {
    let days: u32 = options.retention_days.saturating_add(spec.ttl_extra_days);
    format!("{} + INTERVAL {days} DAY", spec.expiry_column)
}

pub(crate) fn json_field(postgres: bool, (column, path): (&str, &str), cast: &str) -> String {
    if postgres {
        format!("({column} #>> '{{{}}}'){cast}", path.replace('.', ","))
    } else {
        format!("json_extract({column}, '$.{path}')")
    }
}

pub(crate) fn relational_cast(postgres: bool, (expression, kind): (&str, &str)) -> String {
    if postgres {
        format!("CAST({expression} AS {kind})")
    } else {
        expression.into()
    }
}

/// The engine clause, replicated with one shard and every replica on a cluster.
pub(crate) fn clickhouse_engine(spec: &TableSpec, options: &DdlOptions) -> String {
    let version: &str = spec.clickhouse_version.unwrap_or_default();
    if options.cluster.is_some() {
        let separator: &str = if version.is_empty() { "" } else { ", " };
        format!(
            "Replicated{}('/clickhouse/tables/{{uuid}}/{{shard}}', '{{replica}}'{separator}{version})",
            spec.clickhouse_engine
        )
    } else {
        format!("{}({version})", spec.clickhouse_engine)
    }
}

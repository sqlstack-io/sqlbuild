//! `add_column_statement`: the additive upgrade of one table by one column.

use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::_helpers::rendering::{
    clickhouse_identifier, clickhouse_table, clickhouse_type, on_cluster, sqlite_type,
};
use crate::schema::models::{ColumnSpec, DdlOptions, Dialect, TableSpec};

/// `ALTER TABLE … ADD COLUMN`; added columns are always nullable so older rows stay valid.
#[must_use]
pub fn add_column_statement(
    dialect: Dialect,
    options: &DdlOptions,
    (spec, column): (&TableSpec, &ColumnSpec),
) -> String {
    match dialect {
        Dialect::Sqlite => format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            sqlite_identifier(spec.name),
            sqlite_identifier(column.name),
            sqlite_type(column.column_type)
        ),
        Dialect::ClickHouse => {
            let nullable: ColumnSpec = ColumnSpec {
                nullable: true,
                ..*column
            };
            format!(
                "ALTER TABLE {}{} ADD COLUMN IF NOT EXISTS {} {}",
                clickhouse_table(spec.name, options),
                on_cluster(options),
                clickhouse_identifier(column.name),
                clickhouse_type(&nullable)
            )
        }
    }
}

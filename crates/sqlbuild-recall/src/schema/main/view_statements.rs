//! `view_statements`: the statements that create the supported views.

use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::_helpers::clickhouse_views::clickhouse_view_bodies;
use crate::schema::_helpers::relational_views::relational_view_bodies;
use crate::schema::_helpers::rendering::{clickhouse_table, on_cluster};
use crate::schema::models::{DdlOptions, Dialect};

/// Each view's name and creation statement. SQLite views are created only when missing; the store drops them first when the schema version rises. ClickHouse views are replaced.
#[must_use]
pub fn view_statements(dialect: Dialect, options: &DdlOptions) -> Vec<(&'static str, String)> {
    match dialect {
        Dialect::Sqlite => relational_view_bodies(false)
            .into_iter()
            .map(|(name, body)| {
                (
                    name,
                    format!(
                        "CREATE VIEW IF NOT EXISTS {} AS\n{body}",
                        sqlite_identifier(name)
                    ),
                )
            })
            .collect(),
        Dialect::ClickHouse => clickhouse_view_bodies(options)
            .into_iter()
            .map(|(name, body)| {
                (
                    name,
                    format!(
                        "CREATE OR REPLACE VIEW {}{} AS\n{body}",
                        clickhouse_table(name, options),
                        on_cluster(options)
                    ),
                )
            })
            .collect(),
    }
}

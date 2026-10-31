//! Control stores expose Recall's supported views on both engines.

use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::_helpers::relational_views::relational_view_bodies;
use crate::schema::main::view_statements::view_statements;
use crate::schema::models::{DdlOptions, Dialect};

/// PostgreSQL validates each view at migration time; SQLite retains its existing view contract.
#[must_use]
pub fn control_view_statements(postgres: bool) -> Vec<String> {
    if postgres {
        relational_view_bodies(true)
            .into_iter()
            .map(|(name, body)| {
                format!(
                    "CREATE OR REPLACE VIEW {} AS\n{body}",
                    sqlite_identifier(name)
                )
            })
            .collect()
    } else {
        view_statements(
            Dialect::Sqlite,
            &DdlOptions {
                database: None,
                retention_days: 30,
                cluster: None,
            },
        )
        .into_iter()
        .map(|(_, statement)| statement)
        .collect()
    }
}

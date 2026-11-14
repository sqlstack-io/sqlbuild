//! Control stores expose Recall's supported views on both engines.

use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::_helpers::relational_views::relational_view_bodies;

/// PostgreSQL validates each view at migration time; SQLite creates each missing view.
#[must_use]
pub fn control_view_statements(postgres: bool) -> Vec<String> {
    let create = if postgres {
        "CREATE OR REPLACE VIEW"
    } else {
        "CREATE VIEW IF NOT EXISTS"
    };
    relational_view_bodies(postgres, true)
        .into_iter()
        .map(|(name, body)| format!("{create} {} AS\n{body}", sqlite_identifier(name)))
        .collect()
}

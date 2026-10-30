//! Embed Recall in the control database so run facts and projections share one transaction.

use sqlbuild_recall::schema::main::control_table_statements::control_table_statements;
use sqlbuild_recall::schema::main::control_view_statements::control_view_statements;

use crate::store::models::ControlStore;

pub(crate) fn recall_schema(store: &ControlStore) -> Vec<String> {
    let postgres = matches!(store, ControlStore::Postgres(_));
    let mut statements = control_table_statements(postgres);
    statements.extend(control_view_statements(postgres));
    statements
}

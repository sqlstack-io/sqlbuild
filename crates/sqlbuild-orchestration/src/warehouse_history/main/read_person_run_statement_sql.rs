//! Statement SQL needs view.sql on every node the run selected, and each read is audited like node SQL.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::{Action, AuditContext, DefaultRole};
use crate::runs::models::RunStatementSql;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::{sensitive_run_read, statement_sql_in};

/// `None` when the run is not here or the statement is not one of its own.
pub async fn read_person_run_statement_sql(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (run, statement_id): ((Uuid, &str, Uuid), &str),
    default_role: DefaultRole,
) -> Result<Option<RunStatementSql>, ControlError> {
    Ok(sensitive_run_read(
        store,
        credentials,
        (run, default_role, Action::ViewSql),
        (
            "sql.read",
            async |transaction, scope| statement_sql_in(transaction, scope, statement_id).await,
            |sql: &Option<RunStatementSql>| json!({"statement_id": statement_id, "found": sql.is_some()}),
        ),
    )
    .await?
    .flatten())
}

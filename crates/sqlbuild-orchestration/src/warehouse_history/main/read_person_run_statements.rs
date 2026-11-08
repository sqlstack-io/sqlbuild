//! A run's statement metadata needs view on every current node; the SQL text has its own read.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::models::RunStatementRecord;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::{begin_run_recall, run_statements_in};

/// `None` when the run is not in this location and target.
pub async fn read_person_run_statements(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (run, limit): ((Uuid, &str, Uuid), u32),
    default_role: DefaultRole,
) -> Result<Option<Vec<RunStatementRecord>>, ControlError> {
    let Some((transaction, scope, _)) =
        begin_run_recall(store, credentials, run, (default_role, None)).await?
    else {
        return Ok(None);
    };
    let (transaction, statements) = run_statements_in(transaction, &scope, limit).await?;
    commit(transaction).await?;
    Ok(Some(statements))
}

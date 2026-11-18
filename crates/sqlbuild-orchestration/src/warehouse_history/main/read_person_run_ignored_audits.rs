//! The audit failures a run and its current group members let through, with the same view as its statements.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::models::RunIgnoredAuditFailure;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::{begin_run_recall, ignored_audits_in};

/// `None` when the run is not in this location and target.
pub async fn read_person_run_ignored_audits(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    run: (Uuid, &str, Uuid),
    default_role: DefaultRole,
) -> Result<Option<Vec<RunIgnoredAuditFailure>>, ControlError> {
    let Some((transaction, _, _)) =
        begin_run_recall(store, credentials, run, (default_role, None)).await?
    else {
        return Ok(None);
    };
    let (transaction, failures) = ignored_audits_in(transaction, run.2).await?;
    commit(transaction).await?;
    Ok(Some(failures))
}

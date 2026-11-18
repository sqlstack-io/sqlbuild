//! The audit failures a run and its current group members let through, with the same view as its statements.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::models::RunIgnoredAuditFailure;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::{ignored_audits_in, run_read};

/// `None` when the run is not in this location and target.
pub async fn read_person_run_ignored_audits(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    run: (Uuid, &str, Uuid),
    default_role: DefaultRole,
) -> Result<Option<Vec<RunIgnoredAuditFailure>>, ControlError> {
    run_read(
        store,
        credentials,
        (run, default_role),
        async |transaction, _| ignored_audits_in(transaction, run.2).await,
    )
    .await
}

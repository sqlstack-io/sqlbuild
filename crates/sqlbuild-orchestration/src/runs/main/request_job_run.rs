//! Direct-store owners queue published jobs without supplying or widening their write scope.

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::manual_job::request_job_run_in;
use crate::runs::models::JobRunRequest;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// The context is a trusted control-store owner, not a role supplied by an HTTP caller.
pub async fn request_job_run(
    store: &ControlStore,
    context: &AuditContext,
    request: &JobRunRequest,
) -> Result<bool, ControlError> {
    let (transaction, applied) = request_job_run_in(
        begin(store, TransactionMode::Write).await?,
        (context, None),
        request,
    )
    .await?;
    commit(transaction).await?;
    Ok(applied)
}

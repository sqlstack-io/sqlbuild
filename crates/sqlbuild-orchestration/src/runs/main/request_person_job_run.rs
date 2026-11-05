//! Browser callers cannot take the trusted local-owner path or supply an execution write set.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::manual_job::request_job_run_in;
use crate::runs::models::JobRunRequest;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn request_person_job_run(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    request: &JobRunRequest,
) -> Result<bool, ControlError> {
    let (transaction, applied) = request_job_run_in(
        begin(store, TransactionMode::Write).await?,
        (context, Some(credentials)),
        request,
    )
    .await?;
    commit(transaction).await?;
    Ok(applied)
}

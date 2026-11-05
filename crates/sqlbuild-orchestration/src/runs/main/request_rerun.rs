//! Trusted local reruns and browser reruns share freezing, idempotence and publication.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::manual_rerun::request_rerun_in;
use crate::runs::models::RerunRequest;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Omit session credentials only for trusted callers that directly own the control store.
pub async fn request_rerun(
    store: &ControlStore,
    credentials: (&AuditContext, Option<Uuid>),
    request: &RerunRequest,
) -> Result<bool, ControlError> {
    let (transaction, applied) = request_rerun_in(
        begin(store, TransactionMode::Write).await?,
        credentials,
        request,
    )
    .await?;
    commit(transaction).await?;
    Ok(applied)
}

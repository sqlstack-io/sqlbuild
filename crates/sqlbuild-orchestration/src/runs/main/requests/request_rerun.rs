//! Trusted local reruns and browser reruns share freezing, idempotence and publication.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::manual_rerun::request_rerun_in;
use crate::runs::models::RerunRequest;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// Omit session credentials only for trusted callers that directly own the control store.
pub async fn request_rerun(
    store: &ControlStore,
    credentials: (&AuditContext, Option<Uuid>),
    request: &RerunRequest,
) -> Result<bool, ControlError> {
    write(store, |transaction| {
        request_rerun_in(transaction, credentials, request)
    })
    .await
}

//! Direct-store owners queue published jobs without supplying or widening their write scope.

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::manual_job::request_job_run_in;
use crate::runs::models::JobRunRequest;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// The context is a trusted control-store owner, not a role supplied by an HTTP caller.
pub async fn request_job_run(
    store: &ControlStore,
    context: &AuditContext,
    request: &JobRunRequest,
) -> Result<bool, ControlError> {
    write(store, |transaction| {
        request_job_run_in(transaction, (context, None), request)
    })
    .await
}

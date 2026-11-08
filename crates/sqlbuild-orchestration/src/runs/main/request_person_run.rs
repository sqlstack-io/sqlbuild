//! Browser callers cannot take the trusted local-owner path or supply an execution write set.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::adhoc::request_adhoc_run_in;
use crate::runs::_helpers::requests::manual_job::request_job_run_in;
use crate::runs::models::PersonRunRequest;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// A person queues a published job or an ad-hoc command; both are authorised on the nodes they would run.
pub async fn request_person_run(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    request: PersonRunRequest<'_>,
) -> Result<bool, ControlError> {
    write(store, |transaction| async move {
        match request {
            PersonRunRequest::Job(request) => {
                request_job_run_in(transaction, (context, Some(credentials)), request).await
            }
            PersonRunRequest::Adhoc(planner, request) => {
                request_adhoc_run_in(transaction, (context, credentials), (planner, request)).await
            }
        }
    })
    .await
}

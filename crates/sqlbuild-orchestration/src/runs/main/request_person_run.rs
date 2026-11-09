//! Browser callers cannot take the trusted local-owner path or supply an execution write set.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::adhoc::request_adhoc_run_in;
use crate::runs::_helpers::requests::cancel::cancel_person_run;
use crate::runs::_helpers::requests::manual_job::request_job_run_in;
use crate::runs::_helpers::requests::plan::request_plan_in;
use crate::runs::models::PersonRunRequest;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// A person queues a published job, an ad-hoc command or a plan, or cancels a run; each is authorised on the nodes it covers.
pub async fn request_person_run(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    request: PersonRunRequest<'_>,
) -> Result<bool, ControlError> {
    match request {
        PersonRunRequest::Cancel(request) => {
            cancel_person_run(store, (context, credentials), request).await
        }
        PersonRunRequest::Job(request) => {
            write(store, |transaction| async move {
                request_job_run_in(transaction, (context, Some(credentials)), request).await
            })
            .await
        }
        PersonRunRequest::Plan(planner, (location_id, target, request)) => {
            write(store, |transaction| async move {
                request_plan_in(
                    transaction,
                    (context, credentials, planner),
                    (location_id, target, request),
                )
                .await
            })
            .await
        }
        PersonRunRequest::Adhoc(planner, request) => {
            write(store, |transaction| async move {
                request_adhoc_run_in(transaction, (context, credentials), (planner, request)).await
            })
            .await
        }
    }
}

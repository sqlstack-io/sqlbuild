//! Commands on runs, triggers and location requests, each with what to say about it.

use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::declarations::main::change_trigger::change_trigger;
use sqlbuild_orchestration::declarations::models::TriggerChangeRequest;
use sqlbuild_orchestration::platform::main::work::location_work::location_work;
use sqlbuild_orchestration::platform::main::work::request_location_work::request_location_work;
use sqlbuild_orchestration::platform::models::LocationWorkRequest;
use sqlbuild_orchestration::runs::main::execution::change_run::change_run;
use sqlbuild_orchestration::runs::main::requests::request_job_run::request_job_run;
use sqlbuild_orchestration::runs::main::requests::request_rerun::request_rerun;
use sqlbuild_orchestration::runs::models::{
    JobRunRequest, RerunRequest, RunAuthority, RunChange, RunChangeRequest,
};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::adhoc::main::adhoc_planner::adhoc_planner;
use crate::native_commands::_helpers::command::admin_actions::name_or_id;
use crate::native_commands::models::AdminDone;

/// A trusted job run, narrowed with the same planner the server resolves ad-hoc commands with.
pub(crate) async fn queue_job_run(
    store: &ControlStore,
    context: &AuditContext,
    request: JobRunRequest,
) -> Result<AdminDone, ControlError> {
    let planner = adhoc_planner();
    let applied = request_job_run(store, (context, Some(planner.as_ref())), &request).await?;
    Ok(AdminDone::RunQueued {
        run_id: request.run_id,
        job: request.job_key,
        target: request.target,
        applied,
    })
}

pub(crate) async fn queue_rerun(
    store: &ControlStore,
    context: &AuditContext,
    request: RerunRequest,
) -> Result<AdminDone, ControlError> {
    Ok(AdminDone::RerunQueued {
        run_id: request.run_id,
        source: request.source_run_id,
        applied: request_rerun(store, (context, None), &request).await?,
    })
}

pub(crate) async fn request_check(
    store: &ControlStore,
    context: &AuditContext,
    request: LocationWorkRequest,
) -> Result<AdminDone, ControlError> {
    let location: String = name_or_id(store, request.location_id).await?;
    Ok(AdminDone::CheckRequested {
        location,
        applied: request_location_work(store, (context, None), &request).await?,
        request_id: request.request_id,
        target: request.target,
    })
}

pub(crate) async fn read_request(
    store: &ControlStore,
    request_id: Uuid,
) -> Result<AdminDone, ControlError> {
    Ok(AdminDone::WorkRead {
        request_id,
        record: location_work(store, request_id).await?.map(Box::new),
        now: chrono::Utc::now(),
    })
}

pub(crate) async fn request_cancellation(
    store: &ControlStore,
    context: &AuditContext,
    (event_id, run_id, reason): (Uuid, Uuid, String),
) -> Result<AdminDone, ControlError> {
    let request = RunChangeRequest {
        event_id,
        run_id,
        claim_id: None,
        change: RunChange::CancelRequested {
            actor: context.actor.clone(),
            reason,
        },
    };
    Ok(AdminDone::CancelRequested {
        event_id,
        run_id,
        applied: change_run(store, RunAuthority::Operator(context), &request).await?,
    })
}

pub(crate) async fn override_trigger(
    store: &ControlStore,
    context: &AuditContext,
    request: TriggerChangeRequest,
) -> Result<AdminDone, ControlError> {
    let applied: bool = change_trigger(store, context, &request).await?;
    Ok(AdminDone::TriggerChanged {
        event_id: request.event_id,
        trigger_id: request.trigger_id,
        change: request.change,
        applied,
    })
}

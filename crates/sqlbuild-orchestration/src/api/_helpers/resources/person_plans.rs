//! A person plans a build command on the location; the plan never becomes a run.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::credentials::models::{AuditContext, AuthenticatedPerson};
use crate::platform::main::read_person_plan::read_person_plan;
use crate::platform::models::{LocationWorkRecord, PlanRequest};
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::models::PersonRunRequest;
use crate::runs::types::CommandPlanner;
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/control/locations/{location_id}/targets/{target}/plans",
    operation_id = "requestPlan",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("x-sqb-csrf" = String, Header)),
    request_body = PlanRequest,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn request(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target)): Path<(Uuid, String)>,
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Json(request): Json<PlanRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_person_run(
            &store,
            (&context, principal),
            PersonRunRequest::Plan(planner.as_ref(), (location_id, &target, &request)),
        )
        .await?,
    }))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}",
    operation_id = "readPlan",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("request_id" = Uuid, Path)),
    responses((status = 200, body = Option<LocationWorkRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn read(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target, request_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<LocationWorkRecord>>, ApiError> {
    Ok(Json(
        read_person_plan(
            &store,
            (&context, principal),
            (location_id, &target, request_id),
        )
        .await?,
    ))
}

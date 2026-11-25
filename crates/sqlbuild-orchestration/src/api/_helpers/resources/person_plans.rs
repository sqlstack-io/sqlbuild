//! A person plans a build command on the location; the plan never becomes a run.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::credentials::models::{AuditContext, AuthenticatedPerson};
use crate::platform::main::person_reads::read_person_plan::read_person_plan;
use crate::platform::main::person_reads::read_person_plan_diff::read_person_plan_diff;
use crate::platform::main::person_reads::read_person_plan_rows::read_person_plan_rows;
use crate::platform::main::person_reads::read_person_plan_summary::read_person_plan_summary;
use crate::platform::models::{
    LocationWorkRecord, PlanDiffQuery, PlanDiffRead, PlanRequest, PlanRows, PlanRowsQuery,
    PlanSummaryRead,
};
use crate::runs::main::requests::request_person_run::request_person_run;
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

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}/summary",
    operation_id = "readPlanSummary",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("request_id" = Uuid, Path)),
    responses((status = 200, body = Option<PlanSummaryRead>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn summary(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target, request_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<PlanSummaryRead>>, ApiError> {
    Ok(Json(
        read_person_plan_summary(
            &store,
            (&context, principal),
            (location_id, &target, request_id),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}/rows",
    operation_id = "readPlanRows",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("request_id" = Uuid, Path),
        ("section" = inline(crate::platform::models::PlanRowSection), Query, description = "The section to page"),
        ("offset" = Option<usize>, Query, description = "Rows to skip; 0 by default"),
        ("limit" = Option<usize>, Query, description = "At most this many rows, capped at 1000"),
        ("name" = Option<String>, Query, description = "Only rows whose name contains this, ignoring case")),
    responses((status = 200, body = Option<PlanRows>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn rows(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target, request_id)), Query(query)): (
        Path<(Uuid, String, Uuid)>,
        Query<PlanRowsQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<PlanRows>>, ApiError> {
    Ok(Json(
        read_person_plan_rows(
            &store,
            (&context, principal),
            (location_id, &target, request_id),
            query,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}/diff",
    operation_id = "readPlanDiff",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("request_id" = Uuid, Path),
        ("model" = String, Query, description = "The planned model whose diff to read")),
    responses((status = 200, body = PlanDiffRead), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "No diff of that model, or no view.sql on every current node")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn diff(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target, request_id)), Query(query)): (
        Path<(Uuid, String, Uuid)>,
        Query<PlanDiffQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<PlanDiffRead>, ApiError> {
    read_person_plan_diff(
        &store,
        (&context, principal),
        ((location_id, &target, request_id), &query.model),
    )
    .await?
    .map(Json)
    .ok_or_else(|| ApiError {
        status: axum::http::StatusCode::NOT_FOUND,
        message: "plan diff not found".into(),
        decision_id: None,
        cause: None,
    })
}

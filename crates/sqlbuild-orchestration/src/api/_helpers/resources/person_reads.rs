//! Person reads of run nodes, triggers and ticks; refusal precedes every lookup, so unknown objects look forbidden.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::ReadQuery;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::declarations::main::list_person_triggers::list_person_triggers;
use crate::declarations::models::TriggerRecord;
use crate::runs::main::list_person_run_nodes::list_person_run_nodes;
use crate::runs::models::RunNodeRecord;
use crate::scheduler::main::list_person_ticks::list_person_ticks;
use crate::scheduler::models::TickRecord;
use crate::store::models::ControlStore;

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes", operation_id = "listPersonRunNodes",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = Vec<RunNodeRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Run not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn run_nodes(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target, run_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RunNodeRecord>>, ApiError> {
    list_person_run_nodes(
        &store,
        (&context, principal),
        (location_id, &target, run_id),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("run not found"))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/triggers", operation_id = "listPersonTriggers",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<TriggerRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn triggers(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<TriggerRecord>>, ApiError> {
    Ok(Json(
        list_person_triggers(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks", operation_id = "listPersonTicks",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("trigger_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent ticks; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<TickRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 404, description = "Trigger not found for a person who may view the whole target")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn ticks(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, trigger_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<ReadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<TickRecord>>, ApiError> {
    list_person_ticks(
        &store,
        (&context, principal),
        (
            (location_id, &target),
            (trigger_id, options.limit.unwrap_or(100)),
        ),
        default_role,
    )
    .await?
    .map(Json)
    .ok_or_else(|| not_found("trigger not found"))
}

fn not_found(message: &str) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        message: message.into(),
        decision_id: None,
    }
}

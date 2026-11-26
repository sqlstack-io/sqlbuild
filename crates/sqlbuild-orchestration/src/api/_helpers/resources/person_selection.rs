//! A person evaluates a selection, reads the values its selectors take and the past runs they name, against the current publication.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::runs::main::requests::evaluate_selection::evaluate_selection;
use crate::runs::main::requests::read_person_past_runs::read_person_past_runs;
use crate::runs::main::requests::read_selection_vocabulary::read_selection_vocabulary;
use crate::runs::models::{
    PastRunsQuery, PastRunsRead, SelectionEvaluation, SelectionQuery, SelectionVocabulary,
};
use crate::runs::types::CommandPlanner;
use crate::store::models::ControlStore;

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/selection",
    operation_id = "evaluateSelection",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("select" = Option<String>, Query, description = "Space-separated selectors, as `sqb build -s` takes them; empty selects every node"),
        ("exclude" = Option<String>, Query, description = "Space-separated selectors, as `--exclude` takes them"),
        ("offset" = Option<u32>, Query, description = "Sorted keys to skip"),
        ("limit" = Option<u32>, Query, description = "Page size; defaults to 1,000, clamped to 1..10,000")),
    responses((status = 200, body = SelectionEvaluation, description = "The selected keys, or why `sqb` refuses the selectors"),
        (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn evaluate(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target)), Query(query)): (Path<(Uuid, String)>, Query<SelectionQuery>),
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Extension(default_role): Extension<DefaultRole>,
) -> Result<Json<SelectionEvaluation>, ApiError> {
    Ok(Json(
        evaluate_selection(
            &store,
            (planner.as_ref(), (&context, principal)),
            (location_id, &target, &query),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/selection/vocabulary",
    operation_id = "readSelectionVocabulary",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = SelectionVocabulary),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn vocabulary(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target)): Path<(Uuid, String)>,
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Extension(default_role): Extension<DefaultRole>,
) -> Result<Json<SelectionVocabulary>, ApiError> {
    Ok(Json(
        read_selection_vocabulary(
            &store,
            (planner.as_ref(), (&context, principal)),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/selection/runs",
    operation_id = "readPastRuns",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("ids" = Option<String>, Query, description = "Comma-separated run IDs, each full or a unique prefix of at least 8 characters")),
    responses((status = 200, body = PastRunsRead, description = "What each run selected, didn't finish and skipped, or why one can't be read"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn past_runs(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target)), Query(query)): (Path<(Uuid, String)>, Query<PastRunsQuery>),
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Extension(default_role): Extension<DefaultRole>,
) -> Result<Json<PastRunsRead>, ApiError> {
    let ids: BTreeSet<String> = query
        .ids
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    Ok(Json(
        read_person_past_runs(
            &store,
            (planner.as_ref(), (&context, principal)),
            (location_id, &target, &ids),
            default_role,
        )
        .await?,
    ))
}

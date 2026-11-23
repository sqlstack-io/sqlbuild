//! A person reads the estimated query cost of a selection, and daily series for groups of it.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::{CostSelectionQuery, CostSeriesQuery};
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_person_cost_selection::read_person_cost_selection;
use crate::warehouse_history::main::read_person_cost_series::read_person_cost_series;
use crate::warehouse_history::models::{CostSelectionRecord, CostSeriesBatch, CostSeriesGroup};

/// Default and largest cost window, in days.
const COST_DAYS: (u32, u32) = (7, 90);

fn days(days: Option<u32>) -> u32 {
    days.unwrap_or(COST_DAYS.0).clamp(1, COST_DAYS.1)
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/cost/selection", operation_id = "readCostSelection",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("days" = Option<u32>, Query, description = "Builds that started in the last this many whole UTC days, today included; defaults to 7, capped at 90"),
        ("select" = Option<String>, Query, description = "Space-separated selectors, as `sqb build -s` takes them; empty selects every node"),
        ("exclude" = Option<String>, Query, description = "Space-separated selectors, as `--exclude` takes them")),
    responses((status = 200, body = CostSelectionRecord, description = "The selection's cost job by node, against the period before, with daily series by job and top-level folder"),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn cost_selection(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target)), Query(query)): (Path<(Uuid, String)>, Query<CostSelectionQuery>),
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Extension(default_role): Extension<DefaultRole>,
) -> Result<Json<CostSelectionRecord>, ApiError> {
    Ok(Json(
        read_person_cost_selection(
            &store,
            (planner.as_ref(), (&context, principal)),
            (location_id, &target),
            (
                (&query.select, &query.exclude),
                days(query.days),
                default_role,
            ),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/cost/series", operation_id = "readCostSeries",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("days" = Option<u32>, Query, description = "As `readCostSelection`"),
        ("select" = Option<String>, Query, description = "As `readCostSelection`"),
        ("exclude" = Option<String>, Query, description = "As `readCostSelection`"),
        ("groups" = String, Query, description = "A JSON array of at most 200 `CostSeriesGroup`s")),
    responses((status = 200, body = CostSeriesBatch, description = "Each group's credits per day, in the groups' order"),
        (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn cost_series(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target)), Query(query)): (Path<(Uuid, String)>, Query<CostSeriesQuery>),
    (Extension(AuthenticatedPerson(principal, _)), Extension(planner)): (
        Extension<AuthenticatedPerson>,
        Extension<Arc<dyn CommandPlanner>>,
    ),
    Extension(default_role): Extension<DefaultRole>,
) -> Result<Json<CostSeriesBatch>, ApiError> {
    let groups: Vec<CostSeriesGroup> = serde_json::from_str(&query.groups)
        .map_err(|error| ControlError::Invalid(format!("groups: {error}")))?;
    Ok(Json(
        read_person_cost_series(
            &store,
            (planner.as_ref(), (&context, principal)),
            (location_id, &target),
            (
                (&query.select, &query.exclude),
                (days(query.days), &groups),
                default_role,
            ),
        )
        .await?,
    ))
}

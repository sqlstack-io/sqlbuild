//! Location credentials can observe only their own runs; they cannot request or mutate work.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::require_location;
use crate::api::errors::ApiError;
use crate::api::models::ReadQuery;
use crate::credentials::models::TokenIdentity;
use crate::runs::main::list_run_nodes::list_run_nodes;
use crate::runs::main::list_runs::list_runs;
use crate::runs::models::{RunNodeRecord, RunRecord};
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/runs",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent runs; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<RunRecord>), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized")), security(("bearer" = [])))]
pub(crate) async fn recent(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Query(options): Query<ReadQuery>,
) -> Result<Json<Vec<RunRecord>>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        list_runs(&store, (location_id, &target), options.limit.unwrap_or(100)).await?,
    ))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/runs/{run_id}/nodes",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = Vec<RunNodeRecord>), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized"), (status = 404, description = "Run not found")),
    security(("bearer" = [])))]
pub(crate) async fn nodes(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target, run_id)): Path<(Uuid, String, Uuid)>,
) -> Result<Json<Vec<RunNodeRecord>>, ApiError> {
    require_location(&identity, location_id)?;
    let transaction = begin(&store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT run_id FROM sqb_runs WHERE run_id=$1 AND location_id=$2 AND target=$3",
        vec![
            Parameter::Id(run_id),
            Parameter::Id(location_id),
            Parameter::Text(target),
        ],
    )
    .await?;
    commit(transaction).await?;
    if rows.is_empty() {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "run not found".into(),
        });
    }
    Ok(Json(list_run_nodes(&store, run_id).await?))
}

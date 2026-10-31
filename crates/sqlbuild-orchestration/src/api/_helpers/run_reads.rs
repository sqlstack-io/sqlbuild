//! Location credentials can observe only their own runs; they cannot request or mutate work.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::{require_location, require_resource};
use crate::api::errors::ApiError;
use crate::api::models::{ReadQuery, ReadResource, RunRecallPath, RunRecallQuery};
use crate::credentials::models::TokenIdentity;
use crate::runs::main::list_run_nodes::list_run_nodes;
use crate::runs::main::list_runs::list_runs;
use crate::runs::models::{RunNodeRecord, RunRecord};
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::list_recall_invocations::list_recall_invocations;
use crate::store::main::query::query;
use crate::store::main::read_recall_view::read_recall_view;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::types::Row;

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
    require_resource(&store, (location_id, &target), (ReadResource::Run, run_id)).await?;
    Ok(Json(list_run_nodes(&store, run_id).await?))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/runs/{run_id}/invocations",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum invocation namespaces; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<serde_json::Value>), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized"), (status = 404, description = "Run not found")),
    security(("bearer" = [])))]
pub(crate) async fn invocations(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target, run_id)): Path<(Uuid, String, Uuid)>,
    Query(options): Query<ReadQuery>,
) -> Result<Json<Vec<Row>>, ApiError> {
    require_location(&identity, location_id)?;
    require_resource(&store, (location_id, &target), (ReadResource::Run, run_id)).await?;
    let namespaces = run_namespaces(&store, run_id).await?;
    Ok(Json(
        list_recall_invocations(&store, &namespaces, options.limit.unwrap_or(100)).await?,
    ))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/runs/{run_id}/recall/{view}",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("run_id" = Uuid, Path),
        ("view" = String, Path), ("invocation" = String, Query, description = "Exact scheduler or worker invocation ID"),
        ("limit" = Option<u32>, Query, description = "Maximum rows; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<serde_json::Value>), (status = 400, description = "Unknown Recall view"),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 404, description = "Run or invocation not found")), security(("bearer" = [])))]
pub(crate) async fn recall(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(RunRecallPath {
        location_id,
        target,
        run_id,
        view,
    }): Path<RunRecallPath>,
    Query(options): Query<RunRecallQuery>,
) -> Result<Json<Vec<Row>>, ApiError> {
    require_location(&identity, location_id)?;
    require_resource(&store, (location_id, &target), (ReadResource::Run, run_id)).await?;
    let namespaces = run_namespaces(&store, run_id).await?;
    if !namespaces.iter().any(|namespace| {
        options.invocation == *namespace || options.invocation.starts_with(&format!("{namespace}/"))
    }) {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "run invocation not found".into(),
        });
    }
    Ok(Json(
        read_recall_view(
            &store,
            (&view, &options.invocation),
            options.limit.unwrap_or(100),
        )
        .await?,
    ))
}

/// Immutable claim evidence remains available after terminal publication removes liveness.
async fn run_namespaces(store: &ControlStore, run_id: Uuid) -> Result<Vec<String>, ApiError> {
    let scheduler = format!("scheduler/{run_id}");
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, claims) = query(
        transaction,
        "SELECT payload FROM recall_events WHERE invocation_id=$1 AND event_type='run_claimed' ORDER BY invocation_sequence DESC LIMIT 1",
        vec![Parameter::Text(scheduler.clone())],
    ).await?;
    commit(transaction).await?;
    let mut namespaces = vec![scheduler];
    if let Some(claim_id) = claims
        .first()
        .and_then(|row| row["payload"]["claim"]["claim_id"].as_str())
    {
        namespaces.push(format!("worker/{claim_id}"));
    }
    Ok(namespaces)
}

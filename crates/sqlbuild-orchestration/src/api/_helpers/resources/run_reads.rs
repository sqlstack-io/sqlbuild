//! Location credentials can observe only their own runs; they cannot request or mutate work.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::{require_location, require_resource};
use crate::api::errors::ApiError;
use crate::api::models::{ReadQuery, ReadResource, RunRecallPath, RunRecallQuery};
use crate::credentials::models::TokenIdentity;
use crate::runs::main::listings::list_run_nodes::list_run_nodes;
use crate::runs::main::listings::list_runs::list_runs;
use crate::runs::main::listings::run_invocations::run_invocations;
use crate::runs::models::{RunNodeRecord, RunRecord};
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::list_recall_invocations::list_recall_invocations;
use crate::store::main::read_recall_view::read_recall_view;
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::types::Row;

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/runs", operation_id = "list_runs",
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

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/runs/{run_id}/recall/{view}", operation_id = "read_run_recall",
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
            decision_id: None,
            cause: None,
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

/// The run's scheduler and worker invocations, read in their own transaction.
async fn run_namespaces(store: &ControlStore, run_id: Uuid) -> Result<Vec<String>, ApiError> {
    let (transaction, namespaces) =
        run_invocations(begin(store, TransactionMode::Read).await?, run_id).await?;
    commit(transaction).await?;
    Ok(namespaces)
}

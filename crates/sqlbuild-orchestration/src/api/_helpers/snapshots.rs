//! A location credential publishes and reads only its own location's snapshots.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::require_location;
use crate::api::errors::ApiError;
use crate::credentials::models::TokenIdentity;
use crate::snapshots::main::current_snapshot::current_snapshot;
use crate::snapshots::main::current_snapshot_record::current_snapshot_record;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::{CurrentSnapshot, SnapshotInput, SnapshotRecord};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/snapshots",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    request_body = SnapshotInput,
    responses((status = 200, body = SnapshotRecord), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 409, description = "Stale generation")), security(("bearer" = [])))]
pub(crate) async fn publish(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(input): Json<SnapshotInput>,
) -> Result<Json<SnapshotRecord>, ApiError> {
    require_location(&identity, location_id)?;
    if input.location_id != location_id || input.target != target {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "snapshot does not match the requested location and target".into(),
        });
    }
    Ok(Json(publish_snapshot(&store, &input).await?))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/snapshots",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = CurrentSnapshot), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized"), (status = 404, description = "No successful snapshot")),
    security(("bearer" = [])))]
pub(crate) async fn current(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
) -> Result<Json<CurrentSnapshot>, ApiError> {
    require_location(&identity, location_id)?;
    let current = current_snapshot(&store, location_id, &target)
        .await?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "no successful snapshot".into(),
        })?;
    Ok(Json(current))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/snapshots/metadata",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = SnapshotRecord), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized"), (status = 404, description = "No successful snapshot")),
    security(("bearer" = [])))]
pub(crate) async fn metadata(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
) -> Result<Json<SnapshotRecord>, ApiError> {
    require_location(&identity, location_id)?;
    let record = current_snapshot_record(&store, location_id, &target)
        .await?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "no successful snapshot".into(),
        })?;
    Ok(Json(record))
}

//! An authenticated location can mirror only its own registered reader's target.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::require_location;
use crate::api::errors::ApiError;
use crate::credentials::models::TokenIdentity;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::ingest_fingerprints::ingest_fingerprints;
use crate::warehouse_metadata::main::ingest_source_freshness::ingest_source_freshness;
use crate::warehouse_metadata::models::{
    FingerprintBatch, SourceFreshnessBatch, WarehouseSyncResponse,
};

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/metadata/source-freshness", operation_id = "report_source_freshness",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = SourceFreshnessBatch,
    responses((status = 200, body = WarehouseSyncResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn report(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(batch): Json<SourceFreshnessBatch>,
) -> Result<Json<WarehouseSyncResponse>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        ingest_source_freshness(&store, (location_id, &target), &batch).await?,
    ))
}

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/metadata/fingerprints",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = FingerprintBatch,
    responses((status = 200, body = WarehouseSyncResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn report_fingerprints(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(batch): Json<FingerprintBatch>,
) -> Result<Json<WarehouseSyncResponse>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        ingest_fingerprints(&store, (location_id, &target), &batch).await?,
    ))
}

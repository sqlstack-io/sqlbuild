//! An authenticated location can mirror only its own registered reader's target.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::require_location;
use crate::api::errors::ApiError;
use crate::credentials::models::TokenIdentity;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::ingest_audit_results::ingest_audit_results;
use crate::warehouse_metadata::main::ingest_fingerprints::ingest_fingerprints;
use crate::warehouse_metadata::main::ingest_source_freshness::ingest_source_freshness;
use crate::warehouse_metadata::main::read_watermark::read_watermark;
use crate::warehouse_metadata::models::{
    AuditResultsBatch, FingerprintBatch, SourceFreshnessBatch, WarehouseSyncResponse,
    WarehouseWatermarkRead,
};

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/metadata/watermark",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = WarehouseWatermarkRead,
    responses((status = 200, body = Option<chrono::DateTime<chrono::Utc>>),
        (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 409, description = "Reader generation fenced")),
    security(("bearer" = [])))]
pub(crate) async fn watermark(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(request): Json<WarehouseWatermarkRead>,
) -> Result<Json<Option<chrono::DateTime<chrono::Utc>>>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        read_watermark(&store, (location_id, &target), &request).await?,
    ))
}

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

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/metadata/audit-results",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = AuditResultsBatch,
    responses((status = 200, body = WarehouseSyncResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn report_audit_results(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(batch): Json<AuditResultsBatch>,
) -> Result<Json<WarehouseSyncResponse>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        ingest_audit_results(&store, (location_id, &target), &batch).await?,
    ))
}

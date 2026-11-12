//! An authenticated location can mirror only its own registered reader's target.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::require_location;
use crate::api::errors::ApiError;
use crate::credentials::models::TokenIdentity;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::ingest::ingest_audit_results::ingest_audit_results;
use crate::warehouse_metadata::main::ingest::ingest_catalog::ingest_catalog;
use crate::warehouse_metadata::main::ingest::ingest_column_migrations::ingest_column_migrations;
use crate::warehouse_metadata::main::ingest::ingest_consumption::ingest_consumption;
use crate::warehouse_metadata::main::ingest::ingest_fingerprints::ingest_fingerprints;
use crate::warehouse_metadata::main::ingest::ingest_janitor_events::ingest_janitor_events;
use crate::warehouse_metadata::main::ingest::ingest_microbatches::ingest_microbatches;
use crate::warehouse_metadata::main::ingest::ingest_migrations::ingest_migrations;
use crate::warehouse_metadata::main::ingest::ingest_node_results::ingest_node_results;
use crate::warehouse_metadata::main::ingest::ingest_old_name_views::ingest_old_name_views;
use crate::warehouse_metadata::main::ingest::ingest_source_freshness::ingest_source_freshness;
use crate::warehouse_metadata::main::ingest::ingest_statistics::ingest_statistics;
use crate::warehouse_metadata::main::location_reads::read_statistics_due::read_statistics_due;
use crate::warehouse_metadata::main::location_reads::read_sync_due::read_sync_due;
use crate::warehouse_metadata::main::location_reads::read_watermark::read_watermark;
use crate::warehouse_metadata::models::{
    MetadataBatch, StatisticsDueRead, StatisticsRead, WarehouseHistoryKind, WarehouseSyncDue,
    WarehouseSyncDueRead, WarehouseSyncResponse, WarehouseWatermarkRead,
};

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/metadata", operation_id = "report_metadata",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = MetadataBatch,
    responses((status = 200, body = WarehouseSyncResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 409, description = "Reader generation fenced")), security(("bearer" = [])))]
pub(crate) async fn report(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(batch): Json<MetadataBatch>,
) -> Result<Json<WarehouseSyncResponse>, ApiError> {
    require_location(&identity, location_id)?;
    let scope = (location_id, target.as_str());
    let response = match batch.kind {
        WarehouseHistoryKind::Fingerprints => ingest_fingerprints(&store, scope, &batch).await,
        WarehouseHistoryKind::SourceFreshness => {
            ingest_source_freshness(&store, scope, &batch).await
        }
        WarehouseHistoryKind::AuditResults => ingest_audit_results(&store, scope, &batch).await,
        WarehouseHistoryKind::JanitorEvents => ingest_janitor_events(&store, scope, &batch).await,
        WarehouseHistoryKind::Migrations => ingest_migrations(&store, scope, &batch).await,
        WarehouseHistoryKind::OldNameViews => ingest_old_name_views(&store, scope, &batch).await,
        WarehouseHistoryKind::Microbatches => ingest_microbatches(&store, scope, &batch).await,
        WarehouseHistoryKind::ColumnMigrations => {
            ingest_column_migrations(&store, scope, &batch).await
        }
        WarehouseHistoryKind::NodeResults => ingest_node_results(&store, scope, &batch).await,
        WarehouseHistoryKind::Consumption => ingest_consumption(&store, scope, &batch).await,
        WarehouseHistoryKind::Catalog => ingest_catalog(&store, scope, &batch).await,
        WarehouseHistoryKind::Statistics => ingest_statistics(&store, scope, &batch).await,
    }?;
    Ok(Json(response))
}

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/metadata/statistics-due",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = StatisticsDueRead,
    responses((status = 200, body = StatisticsRead), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 409, description = "Reader generation fenced")), security(("bearer" = [])))]
pub(crate) async fn statistics_due(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(request): Json<StatisticsDueRead>,
) -> Result<Json<StatisticsRead>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        read_statistics_due(&store, (location_id, &target), &request).await?,
    ))
}

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

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/metadata/sync-due",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = WarehouseSyncDueRead,
    responses((status = 200, body = WarehouseSyncDue), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 409, description = "Reader generation fenced")),
    security(("bearer" = [])))]
pub(crate) async fn sync_due(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(request): Json<WarehouseSyncDueRead>,
) -> Result<Json<WarehouseSyncDue>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        read_sync_due(&store, (location_id, &target), &request).await?,
    ))
}

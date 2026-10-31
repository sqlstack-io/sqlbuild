//! Trigger definitions and immutable history are visible only within the authenticated location.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::{require_location, require_resource};
use crate::api::errors::ApiError;
use crate::api::models::{ReadQuery, ReadResource};
use crate::credentials::models::TokenIdentity;
use crate::declarations::main::list_trigger_events::list_trigger_events;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::{TriggerChangeRecord, TriggerRecord};
use crate::scheduler::main::list_ticks::list_ticks;
use crate::scheduler::models::TickRecord;
use crate::store::models::ControlStore;

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/triggers",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<TriggerRecord>), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized")), security(("bearer" = [])))]
pub(crate) async fn current(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
) -> Result<Json<Vec<TriggerRecord>>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(list_triggers(&store, location_id, &target).await?))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/events",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("trigger_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum events; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<TriggerChangeRecord>), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized"), (status = 404, description = "Trigger not found")),
    security(("bearer" = [])))]
pub(crate) async fn events(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target, trigger_id)): Path<(Uuid, String, Uuid)>,
    Query(options): Query<ReadQuery>,
) -> Result<Json<Vec<TriggerChangeRecord>>, ApiError> {
    require_location(&identity, location_id)?;
    require_resource(
        &store,
        (location_id, &target),
        (ReadResource::Trigger, trigger_id),
    )
    .await?;
    Ok(Json(
        list_trigger_events(&store, trigger_id, options.limit.unwrap_or(100)).await?,
    ))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("trigger_id" = Uuid, Path),
        ("limit" = Option<u32>, Query, description = "Maximum recent ticks; defaults to 100, capped at 1000")),
    responses((status = 200, body = Vec<TickRecord>), (status = 401, description = "Invalid credential"),
        (status = 403, description = "Location not authorized"), (status = 404, description = "Trigger not found")),
    security(("bearer" = [])))]
pub(crate) async fn ticks(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target, trigger_id)): Path<(Uuid, String, Uuid)>,
    Query(options): Query<ReadQuery>,
) -> Result<Json<Vec<TickRecord>>, ApiError> {
    require_location(&identity, location_id)?;
    require_resource(
        &store,
        (location_id, &target),
        (ReadResource::Trigger, trigger_id),
    )
    .await?;
    Ok(Json(
        list_ticks(&store, trigger_id, options.limit.unwrap_or(100)).await?,
    ))
}

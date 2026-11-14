//! Scope both observations and reads before exposing any location history.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::require_location;
use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::credentials::models::TokenIdentity;
use crate::platform::main::location_health::location_health;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{
    LocationHealthObservation, LocationHealthQuery, LocationHealthReport,
};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/locations/{location_id}/health", operation_id = "report_location_health",
    params(("location_id" = Uuid, Path)), request_body = LocationHealthReport,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn report(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(location_id): Path<Uuid>,
    Json(report): Json<LocationHealthReport>,
) -> Result<Json<MutationResponse>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(MutationResponse {
        applied: report_location_health(&store, location_id, &report).await?,
    }))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/health", operation_id = "read_location_health",
    params(("location_id" = Uuid, Path),
        ("target" = Option<String>, Query, description = "Only this target's observations")),
    responses((status = 200, body = Vec<LocationHealthObservation>),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn recent(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    (Path(location_id), Query(options)): (Path<Uuid>, Query<LocationHealthQuery>),
) -> Result<Json<Vec<LocationHealthObservation>>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(
        location_health(&store, (location_id, options.target.as_deref())).await?,
    ))
}

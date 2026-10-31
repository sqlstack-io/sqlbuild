//! Locations may poll and complete only their own work; enqueueing stays a trusted-owner action.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::require_location;
use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::credentials::models::TokenIdentity;
use crate::platform::main::claim_location_work::claim_location_work;
use crate::platform::main::complete_location_work::complete_location_work;
use crate::platform::main::location_work::location_work;
use crate::platform::models::{
    LocationWorkClaim, LocationWorkPoll, LocationWorkRecord, LocationWorkResult,
};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/locations/{location_id}/work/poll",
    params(("location_id" = Uuid, Path)), request_body = LocationWorkPoll,
    responses((status = 200, body = Option<LocationWorkClaim>), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn poll(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path(location_id): Path<Uuid>,
    Json(poll): Json<LocationWorkPoll>,
) -> Result<Json<Option<LocationWorkClaim>>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(claim_location_work(&store, location_id, &poll).await?))
}

#[utoipa::path(post, path = "/api/locations/{location_id}/work/{request_id}",
    params(("location_id" = Uuid, Path), ("request_id" = Uuid, Path)), request_body = LocationWorkResult,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 409, description = "Work claim expired or was replaced")), security(("bearer" = [])))]
pub(crate) async fn complete(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, request_id)): Path<(Uuid, Uuid)>,
    Json(result): Json<LocationWorkResult>,
) -> Result<Json<MutationResponse>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(MutationResponse {
        applied: complete_location_work(&store, (location_id, request_id), &result).await?,
    }))
}

#[utoipa::path(get, path = "/api/locations/{location_id}/work/{request_id}",
    params(("location_id" = Uuid, Path), ("request_id" = Uuid, Path)),
    responses((status = 200, body = Option<LocationWorkRecord>),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized")),
    security(("bearer" = [])))]
pub(crate) async fn result(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, request_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Option<LocationWorkRecord>>, ApiError> {
    require_location(&identity, location_id)?;
    Ok(Json(location_work(&store, request_id).await?.filter(
        |record| record.request_json.location_id == location_id,
    )))
}

//! An authenticated location reports what the default `sqb build --changed` runs for its own registered reader's target.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::bearer::require_location;
use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::credentials::models::TokenIdentity;
use crate::node_states::main::report_changed_build::report_changed_build;
use crate::node_states::models::ChangedBuildReport;
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/locations/{location_id}/targets/{target}/changed-build", operation_id = "report_changed_build",
    params(("location_id" = Uuid, Path), ("target" = String, Path)), request_body = ChangedBuildReport,
    responses((status = 200, body = MutationResponse, description = "Stored; it replaces the target's previous report"),
        (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Invalid credential"), (status = 403, description = "Location not authorized"),
        (status = 409, description = "Reader generation fenced")),
    security(("bearer" = [])))]
pub(crate) async fn report(
    State(store): State<ControlStore>,
    Extension(identity): Extension<TokenIdentity>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Json(report): Json<ChangedBuildReport>,
) -> Result<Json<MutationResponse>, ApiError> {
    require_location(&identity, location_id)?;
    report_changed_build(&store, (location_id, &target), &report).await?;
    Ok(Json(MutationResponse { applied: true }))
}

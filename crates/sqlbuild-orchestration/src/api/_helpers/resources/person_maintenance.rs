//! Maintenance history and microbatch coverage reads use live person grants over the whole target.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_person_maintenance::read_person_maintenance;
use crate::warehouse_history::main::read_person_microbatch_coverage::read_person_microbatch_coverage;
use crate::warehouse_history::models::{
    CoverageQuery, MaintenanceHistory, MaintenanceQuery, MicrobatchCoverage,
};

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/maintenance", operation_id = "readPersonMaintenance",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("node" = Option<String>, Query, description = "Only moves into this model, and no janitor events"),
        ("limit" = Option<u32>, Query, description = "Maximum moves and janitor events each; defaults to 100, capped at 1000")),
    responses((status = 200, body = MaintenanceHistory), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn read(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), Query(options)): (Path<(Uuid, String)>, Query<MaintenanceQuery>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<MaintenanceHistory>, ApiError> {
    Ok(Json(
        read_person_maintenance(
            &store,
            (&context, principal),
            (location_id, &target, &options),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/microbatches/{model}", operation_id = "readPersonMicrobatchCoverage",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("model" = String, Path),
        ("limit" = Option<u32>, Query, description = "Maximum newest batches; defaults to 200, capped at 1000")),
    responses((status = 200, body = Option<MicrobatchCoverage>, description = "Null when the model has no tracked coverage"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn coverage(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, model)), Query(options)): (
        Path<(Uuid, String, String)>,
        Query<CoverageQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<MicrobatchCoverage>>, ApiError> {
    Ok(Json(
        read_person_microbatch_coverage(
            &store,
            (&context, principal),
            ((location_id, &target), (&model, &options)),
            default_role,
        )
        .await?,
    ))
}

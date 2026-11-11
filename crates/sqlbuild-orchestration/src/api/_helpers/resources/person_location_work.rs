//! Person connection tests and warehouse syncs reuse the location's named outbound work, not arbitrary commands.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::credentials::models::AuditContext;
use crate::credentials::models::AuthenticatedPerson;
use crate::platform::main::read_person_connection_test::read_person_connection_test;
use crate::platform::main::request_location_work::request_location_work;
use crate::platform::models::{
    LocationWork, LocationWorkRecord, LocationWorkRequest, PersonWorkRequest,
};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/control/locations/{location_id}/targets/{target}/connection-tests",
    operation_id = "requestConnectionTest",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("x-sqb-csrf" = String, Header)),
    request_body = PersonWorkRequest,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn request(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<PersonWorkRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    request_work(
        (&store, &context, principal),
        (location_id, target),
        request,
        LocationWork::CheckConnection {},
    )
    .await
}

#[utoipa::path(post, path = "/api/control/locations/{location_id}/targets/{target}/warehouse-syncs",
    operation_id = "requestWarehouseSync",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("x-sqb-csrf" = String, Header)),
    request_body = PersonWorkRequest,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn request_sync(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<PersonWorkRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    request_work(
        (&store, &context, principal),
        (location_id, target),
        request,
        LocationWork::SyncWarehouse {},
    )
    .await
}

/// Both requests need connection-test authority: each makes the location read the warehouse, never write it.
async fn request_work(
    (store, context, principal): (&ControlStore, &AuditContext, Uuid),
    (location_id, target): (Uuid, String),
    request: PersonWorkRequest,
    work: LocationWork,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_location_work(
            store,
            (context, Some(principal)),
            &LocationWorkRequest {
                request_id: request.request_id,
                location_id,
                target,
                reason: request.reason,
                work,
            },
        )
        .await?,
    }))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/connection-tests/{request_id}",
    operation_id = "readConnectionTest",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("request_id" = Uuid, Path)),
    responses((status = 200, body = Option<LocationWorkRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn result(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target, request_id)): Path<(Uuid, String, Uuid)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<LocationWorkRecord>>, ApiError> {
    Ok(Json(
        read_person_connection_test(
            &store,
            (&context, principal),
            (location_id, &target, request_id),
        )
        .await?,
    ))
}

//! Person connection tests reuse the location's named outbound work, not arbitrary commands.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::password_transport::session_credentials;
use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::credentials::models::AuditContext;
use crate::platform::main::read_person_connection_test::read_person_connection_test;
use crate::platform::main::request_location_work::request_location_work;
use crate::platform::models::{
    ConnectionTestRequest, LocationWork, LocationWorkRecord, LocationWorkRequest,
};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/control/locations/{location_id}/targets/{target}/connection-tests",
    operation_id = "requestConnectionTest",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("x-sqb-csrf" = String, Header)),
    request_body = ConnectionTestRequest,
    responses((status = 200, body = MutationResponse), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn request(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(request): Json<ConnectionTestRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: request_location_work(
            &store,
            (&context, Some(session_credentials(&headers))),
            &LocationWorkRequest {
                request_id: request.request_id,
                location_id,
                target,
                reason: request.reason,
                work: LocationWork::CheckConnection {},
            },
        )
        .await?,
    }))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/connection-tests/{request_id}",
    operation_id = "readConnectionTest",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("request_id" = Uuid, Path),
        ("x-sqb-csrf" = String, Header)),
    responses((status = 200, body = Option<LocationWorkRecord>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn result(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target, request_id)): Path<(Uuid, String, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Option<LocationWorkRecord>>, ApiError> {
    Ok(Json(
        read_person_connection_test(
            &store,
            (&context, session_credentials(&headers)),
            (location_id, &target, request_id),
        )
        .await?,
    ))
}

//! Reauthenticate membership changes inside the serialized mutation, not only in middleware.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::password_transport::session_credentials;
use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::authorization::main::assign_builtin_role::assign_builtin_role;
use crate::authorization::main::change_administrator::change_administrator;
use crate::credentials::models::{AdministratorChange, AuditContext, RoleAssignmentRequest};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/admin/people/{principal_id}", operation_id = "changeAdministrator",
    params(("principal_id" = Uuid, Path), ("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    request_body = AdministratorChange,
    responses((status = 200, body = MutationResponse), (status = 401, body = crate::api::models::AuthenticationDenial),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = [])))]
pub(crate) async fn change(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path(principal_id): Path<Uuid>,
    headers: HeaderMap,
    Json(change): Json<AdministratorChange>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: change_administrator(
            &store,
            &context,
            session_credentials(&headers),
            (principal_id, change),
        )
        .await?,
    }))
}

#[utoipa::path(post, path = "/api/admin/roles", operation_id = "assignBuiltinRole",
    params(("x-sqb-csrf" = String, Header)), request_body = RoleAssignmentRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = [])))]
pub(crate) async fn assign(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    headers: HeaderMap,
    Json(request): Json<RoleAssignmentRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: assign_builtin_role(&store, (&context, session_credentials(&headers)), &request)
            .await?,
    }))
}

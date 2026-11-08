//! Reauthenticate membership changes inside the serialized mutation, not only in middleware.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::MutationResponse;
use crate::authorization::main::assign_builtin_role::assign_builtin_role;
use crate::authorization::main::change_administrator::change_administrator;
use crate::authorization::main::list_people::list_people;
use crate::authorization::main::list_role_holdings::list_role_holdings;
use crate::credentials::models::AuthenticatedPerson;
use crate::credentials::models::{
    AdministratorChange, AuditContext, PersonSummary, RoleAssignmentRequest, RoleHoldings,
};
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/admin/people/{principal_id}", operation_id = "changeAdministrator",
    params(("principal_id" = Uuid, Path), ("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    request_body = AdministratorChange,
    responses((status = 200, body = MutationResponse), (status = 401, body = crate::api::models::AuthenticationDenial),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn change(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path(principal_id): Path<Uuid>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(change): Json<AdministratorChange>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: change_administrator(&store, (&context, principal), (principal_id, change))
            .await?,
    }))
}

#[utoipa::path(post, path = "/api/admin/roles", operation_id = "assignBuiltinRole",
    params(("x-sqb-csrf" = String, Header)), request_body = RoleAssignmentRequest,
    responses((status = 200, body = MutationResponse), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn assign(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<RoleAssignmentRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    Ok(Json(MutationResponse {
        applied: assign_builtin_role(&store, (&context, principal), &request).await?,
    }))
}

#[utoipa::path(get, path = "/api/admin/people", operation_id = "listPeople",
    responses((status = 200, body = Vec<PersonSummary>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn people(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<PersonSummary>>, ApiError> {
    Ok(Json(
        list_people(&store, (&context, principal), chrono::Utc::now()).await?,
    ))
}

#[utoipa::path(get, path = "/api/admin/roles", operation_id = "listRoleHoldings",
    responses((status = 200, body = RoleHoldings), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn holdings(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<RoleHoldings>, ApiError> {
    Ok(Json(
        list_role_holdings(&store, (&context, principal)).await?,
    ))
}

//! Service account administration; each change authorizes inside its own write.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::authorization::main::service_accounts::{
    create_service_account::create_service_account, list_service_accounts::list_service_accounts,
    revoke_service_key::revoke_service_key, rotate_service_key::rotate_service_key,
    set_service_account_disabled::set_service_account_disabled,
    set_service_account_role::set_service_account_role,
};
use crate::credentials::models::{
    AuditContext, AuthenticatedPerson, IssuedServiceKey, NewServiceAccount,
    ServiceAccountAuthority, ServiceAccountRole, ServiceAccountState, ServiceAccountSummary,
    ServiceKeyRotation,
};
use crate::store::models::ControlStore;

type Caller = (Extension<AuditContext>, Extension<AuthenticatedPerson>);

fn authority(
    (Extension(context), Extension(AuthenticatedPerson(principal, _))): Caller,
) -> (AuditContext, ServiceAccountAuthority) {
    (context, ServiceAccountAuthority::Person(principal))
}

#[utoipa::path(get, path = "/api/admin/service-accounts", operation_id = "listServiceAccounts",
    description = "Every service account by name with its working keys, never the keys themselves. Needs user.manage or role.assign.",
    responses((status = 200, body = Vec<ServiceAccountSummary>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = []), ("serviceKey" = [])))]
pub(crate) async fn list(
    State(store): State<ControlStore>,
    caller: Caller,
) -> Result<Json<Vec<ServiceAccountSummary>>, ApiError> {
    let (context, authority) = authority(caller);
    Ok(Json(
        list_service_accounts(&store, (&context, authority)).await?,
    ))
}

#[utoipa::path(post, path = "/api/admin/service-accounts", operation_id = "createServiceAccount",
    description = "Adds a service account with a built-in role everywhere and returns its first key, only this once. Needs user.manage and role.assign.",
    params(("x-sqb-csrf" = String, Header, description = "Session CSRF token")), request_body = NewServiceAccount,
    responses((status = 200, body = IssuedServiceKey), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = []), ("serviceKey" = [])))]
pub(crate) async fn create(
    State(store): State<ControlStore>,
    caller: Caller,
    Json(request): Json<NewServiceAccount>,
) -> Result<Json<IssuedServiceKey>, ApiError> {
    let (context, authority) = authority(caller);
    Ok(Json(
        create_service_account(&store, (&context, authority), (&request, None)).await?,
    ))
}

#[utoipa::path(post, path = "/api/admin/service-accounts/{principal_id}/keys", operation_id = "rotateServiceAccountKey",
    description = "Issues a new key, returned only this once. The replaced key works on for overlap_minutes (at most 1440), else stops now; a key already in an overlap stops now. Needs user.manage.",
    params(("principal_id" = Uuid, Path), ("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    request_body = ServiceKeyRotation,
    responses((status = 200, body = IssuedServiceKey), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = []), ("serviceKey" = [])))]
pub(crate) async fn rotate(
    State(store): State<ControlStore>,
    Path(principal_id): Path<Uuid>,
    caller: Caller,
    Json(rotation): Json<ServiceKeyRotation>,
) -> Result<Json<IssuedServiceKey>, ApiError> {
    let (context, authority) = authority(caller);
    Ok(Json(
        rotate_service_key(&store, (&context, authority), (principal_id, rotation)).await?,
    ))
}

#[utoipa::path(post, path = "/api/admin/service-accounts/{principal_id}/keys/{key_id}/revoke", operation_id = "revokeServiceAccountKey",
    description = "Stops one working key now, such as an old key before its overlap ends. Needs user.manage.",
    params(("principal_id" = Uuid, Path), ("key_id" = Uuid, Path), ("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    responses((status = 200, body = ServiceAccountSummary), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = []), ("serviceKey" = [])))]
pub(crate) async fn revoke(
    State(store): State<ControlStore>,
    Path((principal_id, key_id)): Path<(Uuid, Uuid)>,
    caller: Caller,
) -> Result<Json<ServiceAccountSummary>, ApiError> {
    let (context, authority) = authority(caller);
    Ok(Json(
        revoke_service_key(&store, (&context, authority), (principal_id, key_id)).await?,
    ))
}

#[utoipa::path(post, path = "/api/admin/service-accounts/{principal_id}/disabled", operation_id = "setServiceAccountDisabled",
    description = "Disabling refuses every key of the account at once; enabling accepts the same keys again. Needs user.manage.",
    params(("principal_id" = Uuid, Path), ("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    request_body = ServiceAccountState,
    responses((status = 200, body = ServiceAccountSummary), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = []), ("serviceKey" = [])))]
pub(crate) async fn disable(
    State(store): State<ControlStore>,
    Path(principal_id): Path<Uuid>,
    caller: Caller,
    Json(state): Json<ServiceAccountState>,
) -> Result<Json<ServiceAccountSummary>, ApiError> {
    let (context, authority) = authority(caller);
    Ok(Json(
        set_service_account_disabled(
            &store,
            (&context, authority),
            (principal_id, state.disabled),
        )
        .await?,
    ))
}

#[utoipa::path(post, path = "/api/admin/service-accounts/{principal_id}/role", operation_id = "setServiceAccountRole",
    description = "Gives the account a built-in role everywhere in place of its other built-in roles everywhere. Needs role.assign.",
    params(("principal_id" = Uuid, Path), ("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    request_body = ServiceAccountRole,
    responses((status = 200, body = ServiceAccountSummary), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = []), ("serviceKey" = [])))]
pub(crate) async fn role(
    State(store): State<ControlStore>,
    Path(principal_id): Path<Uuid>,
    caller: Caller,
    Json(request): Json<ServiceAccountRole>,
) -> Result<Json<ServiceAccountSummary>, ApiError> {
    let (context, authority) = authority(caller);
    Ok(Json(
        set_service_account_role(&store, (&context, authority), (principal_id, request.role))
            .await?,
    ))
}

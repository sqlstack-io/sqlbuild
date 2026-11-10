//! Reauthenticate membership changes inside the serialized mutation, not only in middleware.

use axum::extract::{Path, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::{
    LoginMode, MutationResponse, ServerFacts, ServerRetention, ServerSettings,
};
use crate::authorization::main::assign_builtin_role::assign_builtin_role;
use crate::authorization::main::change_administrator::change_administrator;
use crate::authorization::main::list_role_definitions::list_role_definitions;
use crate::authorization::main::list_role_holdings::list_role_holdings;
use crate::credentials::main::identities::list_people::list_people;
use crate::credentials::models::AuthenticatedPerson;
use crate::credentials::models::{
    AdministratorChange, AuditContext, PersonSummary, RoleAssignmentRequest, RoleHoldings,
    RoleOverview,
};
use crate::platform::constants::{
    HEARTBEAT_RETENTION_DAYS, LOCATION_RETENTION_DAYS, PLATFORM_ALERT_RETENTION_DAYS,
    SENSOR_EVALUATION_RETENTION_DAYS,
};
use crate::platform::main::read_installation::read_installation;
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

#[utoipa::path(get, path = "/api/admin/locations/{location_id}/targets/{target}/roles", operation_id = "listRoleDefinitions",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<RoleOverview>, description = "Built-in roles, then the target's published custom roles"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn definitions(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<RoleOverview>>, ApiError> {
    Ok(Json(
        list_role_definitions(&store, (&context, principal), (location_id, &target)).await?,
    ))
}

#[utoipa::path(get, path = "/api/admin/server", operation_id = "readServerSettings",
    responses((status = 200, body = ServerSettings), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn server(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Extension(facts), Extension(login_mode)): (Extension<ServerFacts>, Extension<LoginMode>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<ServerSettings>, ApiError> {
    let installation = read_installation(&store, (&context, principal)).await?;
    Ok(Json(ServerSettings {
        version: facts.version,
        login_mode,
        scheduler: facts.scheduler,
        installation,
        retention: ServerRetention {
            audit_days: i64::from(facts.audit.kept_days()),
            statistics_days: i64::from(facts.statistics.kept_days()),
            snapshots_kept: i64::from(facts.snapshots.kept_count()),
            platform_alert_days: PLATFORM_ALERT_RETENTION_DAYS,
            heartbeat_days: HEARTBEAT_RETENTION_DAYS,
            location_health_days: LOCATION_RETENTION_DAYS,
            sensor_evaluation_days: SENSOR_EVALUATION_RETENTION_DAYS,
        },
    }))
}

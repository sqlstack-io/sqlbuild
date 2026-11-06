//! Check reads use live person grants; SQL needs its own permission and is audited.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::ReadQuery;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::read_person_check::read_person_check;
use crate::warehouse_metadata::main::read_person_check_sql::read_person_check_sql;
use crate::warehouse_metadata::main::read_person_checks::read_person_checks;
use crate::warehouse_metadata::models::{CheckResult, CheckSql};

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/checks", operation_id = "listPersonChecks",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Vec<CheckResult>, description = "The newest result of every check"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn list(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<CheckResult>>, ApiError> {
    Ok(Json(
        read_person_checks(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}", operation_id = "readPersonCheckHistory",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("binding_key" = String, Path),
        ("limit" = Option<u32>, Query, description = "Maximum results, newest first; defaults to 50, capped at 1000")),
    responses((status = 200, body = Vec<CheckResult>, description = "Empty for an unknown check"),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn history(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target, binding_key)), Query(options)): (
        Path<(Uuid, String, String)>,
        Query<ReadQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Vec<CheckResult>>, ApiError> {
    Ok(Json(
        read_person_check(
            &store,
            (&context, principal),
            (location_id, &target, &binding_key),
            (options.limit.unwrap_or(50), default_role),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/sql", operation_id = "readPersonCheckSql",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("binding_key" = String, Path)),
    responses((status = 200, body = Option<CheckSql>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn sql(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target, binding_key)): Path<(Uuid, String, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<CheckSql>>, ApiError> {
    Ok(Json(
        read_person_check_sql(
            &store,
            (&context, principal),
            (location_id, &target, &binding_key),
        )
        .await?,
    ))
}

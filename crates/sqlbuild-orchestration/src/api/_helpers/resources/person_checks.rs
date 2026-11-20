//! Check reads use live person grants; the list is served from the held copy with its ETag, and SQL and evidence need their own permissions and are audited.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::resources::cached_json::cached_json;
use crate::api::errors::ApiError;
use crate::api::models::{CheckResultPath, ReadQuery};
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::node_states::main::list_person_checks::list_person_checks;
use crate::node_states::models::NodeStateCache;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_person_check::read_person_check;
use crate::warehouse_history::main::read_person_check_evidence::read_person_check_evidence;
use crate::warehouse_history::main::read_person_check_sql::read_person_check_sql;
use crate::warehouse_history::models::{CheckEvidence, CheckResult, CheckSql};

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/checks", operation_id = "listPersonChecks",
    params(("location_id" = Uuid, Path), ("target" = String, Path),
        ("If-None-Match" = Option<String>, Header, description = "An ETag from an earlier response; 304 while every check's newest result is unchanged")),
    responses((status = 200, body = Vec<CheckResult>, description = "The newest result of every check",
            headers(("ETag" = String, description = "\"<snapshot_id>:<digest of the body>\", or \"<digest>\" before the first publication"))),
        (status = 304, description = "Unchanged since the If-None-Match ETag", headers(("ETag" = String))),
        (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn list(
    (Extension(cache), Extension(context)): (Extension<NodeStateCache>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    (Path((location_id, target)), headers): (Path<(Uuid, String)>, HeaderMap),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Response, ApiError> {
    let listing = list_person_checks(
        &cache,
        (&context, principal),
        (location_id, &target),
        default_role,
    )
    .await?;
    cached_json(listing, &headers)
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

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/results/{event_id}/evidence", operation_id = "readPersonCheckEvidence",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("binding_key" = String, Path), ("event_id" = Uuid, Path)),
    responses((status = 200, body = Option<CheckEvidence>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn evidence(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path(path): Path<CheckResultPath>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<CheckEvidence>>, ApiError> {
    Ok(Json(
        read_person_check_evidence(
            &store,
            (&context, principal),
            (path.location_id, &path.target, &path.binding_key),
            path.event_id,
        )
        .await?,
    ))
}

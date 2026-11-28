//! A person, or a script with their session, asks a location to publish again and reads back its sets.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::errors::ApiError;
use crate::api::models::RepublishWaitQuery;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole};
use crate::platform::main::person_reads::read_person_discovered::read_person_discovered;
use crate::platform::main::work::read_republish::read_republish;
use crate::platform::main::work::request_republish::request_republish;
use crate::platform::models::{DiscoveredReadback, PersonWorkRequest, RepublishStatus};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

#[utoipa::path(post, path = "/api/control/locations/{location_id}/targets/{target}/republish",
    operation_id = "requestRepublish",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("x-sqb-csrf" = String, Header),
        ("wait" = Option<u64>, Query, description = "Seconds to wait for the new snapshot or the publish error, at most 60; 0 or absent returns at once")),
    request_body = PersonWorkRequest,
    responses((status = 200, body = RepublishStatus), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody),
        (status = 409, description = "The location isn't checking in", body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn request(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target)), Query(options)): (
        Path<(Uuid, String)>,
        Query<RepublishWaitQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Json(request): Json<PersonWorkRequest>,
) -> Result<Json<RepublishStatus>, ApiError> {
    request_republish(
        &store,
        (&context, Some(principal)),
        (location_id, &target),
        &request,
    )
    .await?;
    let status = read_republish(
        &store,
        (&context, Some(principal)),
        (location_id, &target, request.request_id),
        options.wait.unwrap_or_default(),
    )
    .await?
    .ok_or_else(|| ControlError::Invalid("the republish request disappeared".into()))?;
    Ok(Json(status))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/republish/{request_id}",
    operation_id = "readRepublish",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("request_id" = Uuid, Path),
        ("wait" = Option<u64>, Query, description = "Seconds to wait while the republish is pending, at most 60")),
    responses((status = 200, body = Option<RepublishStatus>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn read(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    (Path((location_id, target, request_id)), Query(options)): (
        Path<(Uuid, String, Uuid)>,
        Query<RepublishWaitQuery>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<RepublishStatus>>, ApiError> {
    Ok(Json(
        read_republish(
            &store,
            (&context, Some(principal)),
            (location_id, &target, request_id),
            options.wait.unwrap_or_default(),
        )
        .await?,
    ))
}

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/discovered",
    operation_id = "readDiscovered",
    params(("location_id" = Uuid, Path), ("target" = String, Path)),
    responses((status = 200, body = Option<DiscoveredReadback>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn discovered(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(default_role): Extension<DefaultRole>,
    Path((location_id, target)): Path<(Uuid, String)>,
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<Option<DiscoveredReadback>>, ApiError> {
    Ok(Json(
        read_person_discovered(
            &store,
            (&context, principal),
            (location_id, &target),
            default_role,
        )
        .await?,
    ))
}

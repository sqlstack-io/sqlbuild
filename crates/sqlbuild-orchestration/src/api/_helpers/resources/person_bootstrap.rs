//! The UI's first read; it reports access for display and never stands in for a request's authorization.

use axum::http::StatusCode;
use axum::{Extension, Json, extract::State};

use crate::api::errors::ApiError;
use crate::api::models::{ControlBootstrap, LoginMode};
use crate::authorization::main::read_person_access::read_person_access;
use crate::credentials::models::{AuthenticatedPerson, DefaultRole};
use crate::store::models::ControlStore;

#[utoipa::path(get, path = "/api/control/bootstrap", operation_id = "readBootstrap",
    responses((status = 200, body = ControlBootstrap), (status = 401, description = "Session rejected or person disabled")),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn read(
    State(store): State<ControlStore>,
    (Extension(default_role), Extension(login_mode)): (
        Extension<DefaultRole>,
        Extension<LoginMode>,
    ),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
) -> Result<Json<ControlBootstrap>, ApiError> {
    let Some(access) = read_person_access(&store, principal, default_role).await? else {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "authenticated active person required".into(),
            decision_id: None,
        });
    };
    Ok(Json(ControlBootstrap { login_mode, access }))
}

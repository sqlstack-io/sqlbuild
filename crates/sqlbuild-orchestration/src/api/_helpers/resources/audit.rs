//! Person sessions read bounded security facts without machine-token or ambient-viewer access.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};

use crate::api::_helpers::authentication::password_transport::session_credentials;
use crate::api::errors::ApiError;
use crate::api::models::ReadAuditQuery;
use crate::authorization::main::read_security_audit::read_security_audit;
use crate::credentials::models::{AuditContext, SecurityAuditEvent};
use crate::store::models::ControlStore;

#[utoipa::path(get, path = "/api/control/audit", operation_id = "readSecurityAudit",
    params(("after" = Option<i64>, Query, description = "Exclusive storage-order cursor; pre-upgrade facts are ordered by timestamp and event ID"), ("limit" = Option<u32>, Query)),
    responses((status = 200, body = Vec<SecurityAuditEvent>), (status = 400, body = crate::api::models::ApiErrorBody),
        (status = 401, description = "Session rejected"), (status = 403, body = crate::api::models::ApiErrorBody)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn read(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Query(options): Query<ReadAuditQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<SecurityAuditEvent>>, ApiError> {
    Ok(Json(
        read_security_audit(
            &store,
            (&context, session_credentials(&headers)),
            (options.after.unwrap_or(0), options.limit.unwrap_or(100)),
        )
        .await?,
    ))
}

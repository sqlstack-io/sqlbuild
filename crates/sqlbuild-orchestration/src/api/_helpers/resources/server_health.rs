//! Liveness and health, which answer from memory without signing in, and health details for administrators.

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};

use crate::api::errors::ApiError;
use crate::credentials::models::{AuditContext, AuthenticatedPerson};
use crate::health::main::health_report::health_report;
use crate::health::main::read_health_details::read_health_details;
use crate::health::models::{HealthDetails, HealthReport, HealthStatus, ServerHealth};
use crate::store::models::ControlStore;

/// `/livez` and `/api/health`, which read only the server's memory.
pub(crate) fn health_routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/livez", get(livez))
        .route("/api/health", get(health))
}

/// The process answers; nothing else is checked.
#[utoipa::path(get, path = "/livez", operation_id = "livez",
    responses((status = 200, description = "The server process is running", body = String)))]
pub(crate) async fn livez() -> Response {
    no_store((StatusCode::OK, "ok\n").into_response())
}

#[utoipa::path(get, path = "/api/health", operation_id = "readHealth",
    responses((status = 200, description = "Every component is healthy", body = HealthReport),
        (status = 503, description = "A component is degraded or the control database is unavailable", body = HealthReport)))]
pub(crate) async fn health(Extension(health): Extension<ServerHealth>) -> Response {
    let report: HealthReport = health_report(&health, chrono::Utc::now());
    let status = if report.status == HealthStatus::Healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    no_store((status, Json(report)).into_response())
}

#[utoipa::path(get, path = "/api/health/details", operation_id = "readHealthDetails",
    responses((status = 200, body = HealthDetails), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn details(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Extension(AuthenticatedPerson(principal, _)): Extension<AuthenticatedPerson>,
    Extension(health): Extension<ServerHealth>,
) -> Result<Json<HealthDetails>, ApiError> {
    Ok(Json(
        read_health_details((&store, &health), (&context, principal)).await?,
    ))
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

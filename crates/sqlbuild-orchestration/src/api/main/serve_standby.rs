//! Answer on the server's address before its control database can be opened: liveness, health and the UI work, and
//! every other API request is refused with the database's cause, until the full API takes the listener over.

use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router, middleware};
use tower_http::compression::CompressionLayer;

use crate::api::_helpers::resources::request_ids::identify_request;
use crate::api::_helpers::resources::server_health::health_routes;
use crate::api::_helpers::resources::ui_assets::{UI_ASSETS, ui_response};
use crate::api::constants::{API_PATH, API_PREFIX};
use crate::api::errors::ApiError;
use crate::health::constants::CONNECTING;
use crate::health::models::{ControlCause, ServerHealth};
use crate::store::errors::ControlError;

/// Serve the standby on `listener` until `done` resolves.
pub async fn serve_standby(
    listener: tokio::net::TcpListener,
    health: ServerHealth,
    done: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ControlError> {
    let application: Router = health_routes()
        .fallback(standby_fallback)
        .layer(Extension(health.clone()))
        .layer(middleware::from_fn_with_state(health, identify_request))
        .layer(CompressionLayer::new());
    axum::serve(
        listener,
        application.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(done)
    .await?;
    Ok(())
}

async fn standby_fallback(
    Extension(health): Extension<ServerHealth>,
    method: Method,
    uri: Uri,
) -> Response {
    let path: &str = uri.path();
    if path != API_PATH && !path.starts_with(API_PREFIX) {
        return ui_response(UI_ASSETS, &method, path);
    }
    let cause: Option<ControlCause> = health.control_cause();
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: cause
            .as_ref()
            .map_or_else(|| CONNECTING.to_owned(), |cause| cause.message.clone()),
        decision_id: None,
        cause,
    }
    .into_response()
}

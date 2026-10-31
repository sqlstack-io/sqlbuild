//! One native server owns migrations and all storage writes; locations and workers speak HTTP.

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::{Json, Router, middleware};

use crate::api::_helpers::authentication::authenticate;
use crate::api::_helpers::{run_reads, runs, snapshots, trigger_reads};
use crate::api::constants::BODY_BYTES;
use crate::api::main::openapi_document::openapi_document;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;

pub async fn serve_api(
    store: ControlStore,
    listener: tokio::net::TcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ControlError> {
    migrate_store(&store).await?;
    let protected = Router::new()
        .route(
            "/api/locations/{location_id}/targets/{target}/snapshots",
            get(snapshots::current).post(snapshots::publish),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/runs",
            get(run_reads::recent),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/runs/{run_id}/nodes",
            get(run_reads::nodes),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/triggers",
            get(trigger_reads::current),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/events",
            get(trigger_reads::events),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks",
            get(trigger_reads::ticks),
        )
        .route("/api/runs/{run_id}/execution", get(runs::execution))
        .route("/api/runs/{run_id}/events", post(runs::report))
        .route("/api/runs/{run_id}/heartbeat", post(runs::heartbeat))
        .route("/api/runs/{run_id}/output", post(runs::output))
        .route("/api/runs/{run_id}/recall", post(runs::recall))
        .layer(DefaultBodyLimit::max(BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(store.clone(), authenticate));
    let application = protected
        .route(
            "/api/openapi.json",
            get(|| async { Json(openapi_document()) }),
        )
        .with_state(store);
    axum::serve(listener, application)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

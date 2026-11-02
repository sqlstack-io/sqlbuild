//! Machine resource routes share bearer authentication and bounded request bodies.

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::{Extension, Router, middleware};

use crate::api::_helpers::authentication::bearer::authenticate;
use crate::api::_helpers::resources::{
    changes, location_health, location_work, run_reads, runs, snapshots, trigger_reads,
    warehouse_metadata,
};
use crate::api::constants::BODY_BYTES;
use crate::api::models::ChangeFeed;
use crate::store::models::ControlStore;

pub(crate) fn router(store: &ControlStore, feed: ChangeFeed) -> Router<ControlStore> {
    Router::new()
        .route(
            "/api/locations/{location_id}/targets/{target}/changes",
            get(changes::subscribe),
        )
        .route(
            "/api/locations/{location_id}/work/poll",
            post(location_work::poll),
        )
        .route(
            "/api/locations/{location_id}/work/{request_id}",
            get(location_work::result).post(location_work::complete),
        )
        .route(
            "/api/locations/{location_id}/health",
            get(location_health::recent).post(location_health::report),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/metadata/source-freshness",
            post(warehouse_metadata::report),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/metadata/fingerprints",
            post(warehouse_metadata::report_fingerprints),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/metadata/watermark",
            post(warehouse_metadata::watermark),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/snapshots",
            get(snapshots::current).post(snapshots::publish),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/snapshots/metadata",
            get(snapshots::metadata),
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
            "/api/locations/{location_id}/targets/{target}/runs/{run_id}/invocations",
            get(run_reads::invocations),
        )
        .route(
            "/api/locations/{location_id}/targets/{target}/runs/{run_id}/recall/{view}",
            get(run_reads::recall),
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
        .layer(Extension(feed))
        .route_layer(middleware::from_fn_with_state(store.clone(), authenticate))
}

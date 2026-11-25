//! The API's routes with the extensions every request reads, and the layers around them.

use std::sync::Arc;

use axum::{Extension, Router, middleware};
use tokio::sync::{broadcast, watch};
use tower_http::compression::CompressionLayer;

use crate::api::_helpers::authentication::bearer::local_request;
use crate::api::_helpers::authentication::development_hosts::development_authorities;
use crate::api::_helpers::resources::compression::compress_reads_only;
use crate::api::_helpers::resources::request_ids::identify_request;
use crate::api::_helpers::resources::router::router;
use crate::api::_helpers::resources::server_errors::generic_server_errors;
use crate::api::_helpers::resources::server_health::health_routes;
use crate::api::_helpers::resources::ui_assets::{UI_ASSETS, serve_ui};
use crate::api::models::{ChangeFeed, DevelopmentScope, ServerFacts};
use crate::health::models::ServerHealth;
use crate::node_states::models::NodeStateCache;
use crate::runs::types::RunHandoff;
use crate::store::models::{ChangeMessage, ControlStore};

/// Every API route over `store`, with the change feed, node states, run handoff and server facts.
pub(crate) fn application_router(
    store: &ControlStore,
    (sender, stopped): (broadcast::Sender<ChangeMessage>, watch::Receiver<bool>),
    authentication_routes: Router<ControlStore>,
    (node_states, (images, planner), facts): (NodeStateCache, RunHandoff, ServerFacts),
) -> Router {
    let feed = ChangeFeed { sender, stopped };
    router(store, feed.clone())
        .merge(health_routes())
        .merge(authentication_routes.layer(Extension(feed)))
        .layer(Extension(facts.health.clone()))
        .layer(Extension(node_states))
        .layer(Extension(Arc::new(images)))
        .layer(Extension(planner))
        .layer(Extension(facts.statistics))
        .layer(Extension(facts))
        .with_state(store.clone())
}

/// The development UI, the Host and Origin guard of a development scope, and the response layers.
pub(crate) fn development_application(
    application: Router,
    (scope, health): (Option<DevelopmentScope>, &ServerHealth),
    address: std::net::SocketAddr,
) -> Router {
    let application = serve_ui(application, UI_ASSETS);
    let application = match scope {
        Some(scope) => {
            let accepted = Arc::new(development_authorities(address, &scope.allowed_hosts));
            application
                .layer(Extension(scope))
                .layer(middleware::from_fn_with_state(accepted, local_request))
        }
        None => application,
    };
    application
        .layer(middleware::map_response(generic_server_errors))
        .layer(middleware::from_fn_with_state(
            health.clone(),
            identify_request,
        ))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(compress_reads_only))
}

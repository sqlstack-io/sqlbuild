//! One native server owns migrations and all storage writes; locations and workers speak HTTP.

use axum::routing::get;
use axum::{Extension, Json, Router, middleware};

use crate::api::_helpers::authentication::{bearer::local_request, configure::configure};
use crate::api::_helpers::resources::server_errors::generic_server_errors;
use crate::api::_helpers::resources::ui_assets::{UI_ASSETS, serve_ui};
use crate::api::_helpers::resources::{changes, router::router};
use crate::api::main::openapi_document::openapi_document;
use crate::api::models::{ApiAuthentication, ChangeFeed, DevelopmentScope, GroupRole, ServerFacts};
use crate::authorization::main::publish_group_roles::publish_group_roles;
use crate::runs::types::RunHandoff;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::subscribe_changes::subscribe_changes;
use crate::store::models::ControlStore;

pub async fn serve_api(
    store: ControlStore,
    listener: tokio::net::TcpListener,
    (authentication, (images, planner), facts): (ApiAuthentication, RunHandoff, ServerFacts),
    (ready, shutdown): (
        impl FnOnce() + Send + 'static,
        impl Future<Output = ()> + Send + 'static,
    ),
) -> Result<(), ControlError> {
    let address = listener.local_addr()?;
    let (development_scope, authentication_routes, group_roles) =
        configure(&store, authentication, address).await?;
    prepare_store(&store, &group_roles).await?;
    let (sender, listener_changes) = subscribe_changes(&store).await?;
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let feed = ChangeFeed {
        sender: sender.clone(),
        stopped,
    };
    let application = router(&store, feed.clone())
        .merge(authentication_routes.layer(Extension(feed)))
        .layer(Extension(std::sync::Arc::new(images)))
        .layer(Extension(planner))
        .layer(Extension(facts.statistics))
        .layer(Extension(facts))
        .route(
            "/api/openapi.json",
            get(|| async { Json(openapi_document()) }),
        )
        .with_state(store.clone());
    let application = development_application(application, development_scope, address);
    let server = axum::serve(
        listener,
        application.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown.await;
        stop.send_replace(true);
    });
    ready();
    tokio::select! {
        result = async { server.await } => { result?; Ok(()) },
        result = changes::relay(store.clone(), listener_changes, sender) => result,
    }
}

async fn prepare_store(
    store: &ControlStore,
    group_roles: &[GroupRole],
) -> Result<(), ControlError> {
    migrate_store(store).await?;
    publish_group_roles(store, group_roles).await
}

fn development_application(
    application: Router,
    scope: Option<DevelopmentScope>,
    address: std::net::SocketAddr,
) -> Router {
    let application = serve_ui(application, UI_ASSETS);
    let application = match scope {
        Some(scope) => application
            .layer(Extension(scope))
            .layer(middleware::from_fn_with_state(address, local_request)),
        None => application,
    };
    application.layer(middleware::map_response(generic_server_errors))
}

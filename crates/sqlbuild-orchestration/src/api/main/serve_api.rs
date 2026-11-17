//! One native server owns migrations and all storage writes; locations and workers speak HTTP.

use axum::Json;
use axum::routing::get;

use crate::api::_helpers::authentication::configure::configure;
use crate::api::_helpers::resources::application::{application_router, development_application};
use crate::api::_helpers::resources::changes;
use crate::api::main::openapi_document::openapi_document;
use crate::api::models::{ApiAuthentication, GroupRole, ServerFacts};
use crate::authorization::main::publish_group_roles::publish_group_roles;
use crate::node_states::main::start_node_states::start_node_states;
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
    let (node_states, refresh_node_states) = start_node_states(&store, &sender);
    let application = application_router(
        &store,
        (sender.clone(), stopped),
        authentication_routes,
        (node_states, (images, planner), facts),
    )
    .route(
        "/api/openapi.json",
        get(|| async { Json(openapi_document()) }),
    );
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
        () = refresh_node_states => Ok(()),
    }
}

async fn prepare_store(
    store: &ControlStore,
    group_roles: &[GroupRole],
) -> Result<(), ControlError> {
    migrate_store(store).await?;
    publish_group_roles(store, group_roles).await
}

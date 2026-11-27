//! One native server owns migrations and all storage writes; locations and workers speak HTTP.

use axum::Json;
use axum::routing::get;

use crate::api::_helpers::authentication::configure::configure;
use crate::api::_helpers::resources::application::{application_router, development_application};
use crate::api::_helpers::resources::changes;
use crate::api::main::openapi_document::openapi_document;
use crate::api::models::{ApiAuthentication, GroupRole, ServerFacts};
use crate::authorization::main::roles::publish_group_roles::publish_group_roles;
use crate::health::main::probe_control_database::probe_control_database;
use crate::health::models::ServerHealth;
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
    let health = facts.health.clone();
    health.attach_store(&store);
    health.control_succeeded(chrono::Utc::now());
    let probing = (health.clone(), stopped.clone());
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
    let application = development_application(application, (development_scope, &health), address);
    let server = axum::serve(
        listener,
        application.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown.await;
        stop.send_replace(true);
    });
    let mut roles = background_roles(
        (store.clone(), listener_changes, sender),
        (refresh_node_states, probing),
    );
    ready();
    tokio::select! {
        result = async { server.await } => { result?; Ok(()) },
        Some(ended) = roles.join_next() => ended?,
    }
}

/// The change relay, the node-states refresh and the database probe, each its own task, so none can stop the accept loop's task from accepting; dropping the set stops them.
fn background_roles(
    (store, listener_changes, sender): (
        ControlStore,
        Option<sqlx_postgres::PgListener>,
        tokio::sync::broadcast::Sender<crate::store::models::ChangeMessage>,
    ),
    (refresh_node_states, (health, stopped)): (
        impl Future<Output = ()> + Send + 'static,
        (ServerHealth, tokio::sync::watch::Receiver<bool>),
    ),
) -> tokio::task::JoinSet<Result<(), ControlError>> {
    let probe = probe_control_database(store.clone(), health, stopped);
    let mut roles = tokio::task::JoinSet::new();
    roles.spawn(changes::relay(store, listener_changes, sender));
    roles.spawn(async move {
        refresh_node_states.await;
        Ok(())
    });
    roles.spawn(async move {
        probe.await;
        Ok(())
    });
    roles
}

async fn prepare_store(
    store: &ControlStore,
    group_roles: &[GroupRole],
) -> Result<(), ControlError> {
    migrate_store(store).await?;
    publish_group_roles(store, group_roles).await
}

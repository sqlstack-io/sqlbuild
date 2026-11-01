//! One native server owns migrations and all storage writes; locations and workers speak HTTP.

use axum::routing::get;
use axum::{Extension, Json, Router, middleware};

use crate::api::_helpers::authentication::{bearer::local_request, configure::configure};
use crate::api::_helpers::resources::{changes, router::router};
use crate::api::main::openapi_document::openapi_document;
use crate::api::models::{ApiAuthentication, ChangeFeed, DevelopmentScope, DockerImageSource};
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::subscribe_changes::subscribe_changes;
use crate::store::models::ControlStore;

pub async fn serve_api(
    store: ControlStore,
    listener: tokio::net::TcpListener,
    (authentication, images): (ApiAuthentication, Vec<DockerImageSource>),
    (ready, shutdown): (
        impl FnOnce() + Send + 'static,
        impl Future<Output = ()> + Send + 'static,
    ),
) -> Result<(), ControlError> {
    let address = listener.local_addr()?;
    let (development_scope, authentication_routes) =
        configure(&store, authentication, address).await?;
    migrate_store(&store).await?;
    let (sender, listener_changes) = subscribe_changes(&store).await?;
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let feed = ChangeFeed {
        sender: sender.clone(),
        stopped,
    };
    let application = router(&store, feed)
        .merge(authentication_routes)
        .layer(Extension(std::sync::Arc::new(images)))
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
        result = changes::relay(store, listener_changes, sender) => result,
    }
}

fn development_application(
    application: Router,
    scope: Option<DevelopmentScope>,
    address: std::net::SocketAddr,
) -> Router {
    match scope {
        Some(scope) => application
            .layer(Extension(scope))
            .layer(middleware::from_fn_with_state(address, local_request)),
        None => application,
    }
}

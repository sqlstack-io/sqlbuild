//! Bind the server's address first and answer there while the control store opens, waiting out a database that can't
//! be reached yet; liveness, health and the UI say why meanwhile.

use std::net::SocketAddr;

use chrono::Utc;
use sqlbuild_orchestration::api::main::serve_standby::serve_standby;
use sqlbuild_orchestration::health::main::control_cause::control_cause;
use sqlbuild_orchestration::health::main::control_target::control_target;
use sqlbuild_orchestration::health::models::ServerHealth;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::command::decision::{
    control_database, control_database_variable,
};
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::open_migrated_store;
use crate::native_commands::constants::{
    OPENING_STORE, STORE_READY, STORE_RETRY_FIRST, STORE_RETRY_MAX,
};
use crate::native_commands::models::ApiStandby;
use crate::server_config::models::ServerSection;

/// Bind `address` and serve the standby on it until the returned standby is ended.
pub(crate) async fn start_standby(
    address: SocketAddr,
    health: &ServerHealth,
) -> Result<ApiStandby, ControlError> {
    drop(write_output(&[(
        Stream::Stderr,
        "Starting control API...\n",
    )]));
    let bound = std::net::TcpListener::bind(address)?;
    bound.set_nonblocking(true)?;
    let standby = tokio::net::TcpListener::from_std(bound.try_clone()?)?;
    let listener = tokio::net::TcpListener::from_std(bound)?;
    drop(write_output(&[(
        Stream::Stderr,
        &format!("Control API listening: {}\n", listener.local_addr()?),
    )]));
    let (end, ended) = tokio::sync::oneshot::channel::<()>();
    let health = health.clone();
    drop(tokio::spawn(async move {
        let done = async move {
            drop(ended.await);
        };
        if let Err(error) = serve_standby(standby, health, done).await {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Control API standby stopped: {error}\n"),
            )]));
        }
    }));
    Ok(ApiStandby { listener, end })
}

/// Open and migrate the server's store, retrying while the database can't be reached and recording why in `health`.
pub(crate) async fn open_store_waiting(
    namespace: &Namespace,
    server: &ServerSection,
    health: &ServerHealth,
) -> Result<ControlStore, ControlError> {
    let url = control_database(namespace, &control_database_variable(namespace, server))?;
    let target = control_target(&url);
    health.set_target(target.clone());
    drop(write_output(&[(Stream::Stderr, OPENING_STORE)]));
    let mut wait = STORE_RETRY_FIRST;
    loop {
        let error = match open_migrated_store(&url).await {
            Ok(store) => {
                drop(write_output(&[(Stream::Stderr, STORE_READY)]));
                return Ok(store);
            }
            Err(error) => error,
        };
        let Some(cause) = control_cause(&error, (&target, false)) else {
            return Err(error);
        };
        let detail = cause.chain.join(": ");
        if !error.transient() {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("{}: {detail}\n", cause.message),
            )]));
            return Err(error);
        }
        health.control_failed(&cause, (Utc::now(), None));
        drop(write_output(&[(
            Stream::Stderr,
            &format!(
                "{} ({detail}); retrying in {} s\n",
                cause.message,
                wait.as_secs()
            ),
        )]));
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(STORE_RETRY_MAX);
    }
}

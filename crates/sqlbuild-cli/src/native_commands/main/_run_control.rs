//! Supervise native API/scheduler roles together; a failed role stops its siblings.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use tokio::sync::watch;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream, Value};
use crate::native_commands::_helpers::decision::{flag, text};
use crate::native_commands::_helpers::terminal::write_output;

async fn stopped(mut shutdown: watch::Receiver<bool>) {
    while !*shutdown.borrow_and_update() {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

async fn shutdown_signal() -> Result<(), ControlError> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result?,
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}

pub(crate) async fn run_control(
    namespace: &Namespace,
    standalone: bool,
) -> Result<(), ControlError> {
    let address = if standalone {
        None
    } else {
        let host: IpAddr = text(namespace, "control_host")
            .unwrap_or_else(|| "127.0.0.1".into())
            .parse()
            .map_err(|_| ControlError::Invalid("--host must be an IP address".into()))?;
        let port = match namespace.get("control_port") {
            Some(Value::Int(value)) => value
                .parse::<u16>()
                .map_err(|_| ControlError::Invalid("--port must be between 0 and 65535".into()))?,
            _ => 8000,
        };
        Some(SocketAddr::new(host, port))
    };
    let variable = text(namespace, "control_database_env")
        .unwrap_or_else(|| "SQLBUILD_CONTROL_DATABASE_URL".into());
    let url = match std::env::var(&variable) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => {
            PathBuf::from(text(namespace, "project_dir").unwrap_or_else(|| ".".into()))
                .join(".sqlbuild/control.sqlite3")
                .to_string_lossy()
                .into_owned()
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(ControlError::Invalid(
                "control database variable must be UTF-8".into(),
            ));
        }
    };
    drop(write_output(&[(
        Stream::Stderr,
        "Opening control store...\n",
    )]));
    let store = open_store(&url).await?;
    migrate_store(&store).await?;
    drop(write_output(&[(Stream::Stderr, "Control store ready\n")]));
    let (stop, shutdown) = watch::channel(false);
    let (_changes, changes) = watch::channel(());
    let mut tasks = tokio::task::JoinSet::new();
    if let Some(address) = address {
        drop(write_output(&[(
            Stream::Stderr,
            "Starting control API...\n",
        )]));
        let listener = tokio::net::TcpListener::bind(address).await?;
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Control API ready: {}\n", listener.local_addr()?),
        )]));
        tasks.spawn(serve_api(
            store.clone(),
            listener,
            stopped(shutdown.clone()),
        ));
    }
    if standalone || !flag(namespace, "control_no_scheduler") {
        drop(write_output(&[(Stream::Stderr, "Starting scheduler...\n")]));
        let scheduler_store = store.clone();
        tasks.spawn(async move {
            run_scheduler(
                &scheduler_store,
                SchedulerState::new(Uuid::now_v7()),
                shutdown,
                changes,
            )
            .await
        });
        drop(write_output(&[(
            Stream::Stderr,
            "Scheduler loop started; leadership is lease-guarded\n",
        )]));
    }
    let mut result = tokio::select! {
        result = tasks.join_next() => match result {
            Some(result) => result.map_err(ControlError::from).and_then(std::convert::identity),
            None => Ok(()),
        },
        result = shutdown_signal() => result,
    };
    stop.send_replace(true);
    while let Some(joined) = tasks.join_next().await {
        let joined = joined
            .map_err(ControlError::from)
            .and_then(std::convert::identity);
        result = result.and(joined);
    }
    result
}

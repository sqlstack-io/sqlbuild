//! Supervise native API/scheduler roles together; a failed role stops its siblings.

use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::platform::main::run_watchdog::run_watchdog;
use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use tokio::sync::watch;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{control_address, control_database, flag};
use crate::native_commands::_helpers::execution::supervise_control_roles;
use crate::native_commands::_helpers::terminal::write_output;

async fn stopped(mut shutdown: watch::Receiver<bool>) {
    while !*shutdown.borrow_and_update() {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

pub(crate) async fn run_control(
    namespace: &Namespace,
    standalone: bool,
) -> Result<(), ControlError> {
    let address = control_address(namespace, standalone)?;
    let url = control_database(namespace)?;
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
        drop(write_output(&[(
            Stream::Stderr,
            "Starting platform watchdog...\n",
        )]));
        let watchdog_store = store.clone();
        let watchdog_shutdown = shutdown.clone();
        tasks.spawn(async move {
            run_watchdog(&watchdog_store, Uuid::now_v7(), watchdog_shutdown).await
        });
        drop(write_output(&[(
            Stream::Stderr,
            "Platform watchdog started\n",
        )]));
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
    supervise_control_roles(tasks, stop).await
}

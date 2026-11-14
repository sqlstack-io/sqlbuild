//! Supervise native API/scheduler roles together; a failed role stops its siblings.

use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::platform::models::RetentionPolicies;
use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::sync::watch;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{control_address, control_api_settings};
use crate::native_commands::_helpers::execution::{
    open_control_store, open_control_workers, start_platform_roles, supervise_control_roles,
};
use crate::native_commands::_helpers::terminal::write_output;
use crate::server_config::main::read_server_config::read_server_config;
use crate::workers::models::Workers;

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
    let config = read_server_config(namespace)?;
    let address = control_address(namespace, standalone, &config.server)?;
    let (authentication, facts) = control_api_settings(namespace, standalone, &config)?;
    let store = open_control_store(namespace, Some(&config.server)).await?;
    let (workers, handoff) = open_control_workers(
        (&store, namespace),
        (&config.server, config.launchers),
        facts.scheduler,
    )
    .await?;
    let (stop, shutdown) = watch::channel(false);
    let mut tasks = JoinSet::new();
    if let Some(address) = address {
        drop(write_output(&[(
            Stream::Stderr,
            "Starting control API...\n",
        )]));
        let listener = tokio::net::TcpListener::bind(address).await?;
        let address = listener.local_addr()?;
        tasks.spawn(serve_api(
            store.clone(),
            listener,
            (authentication, handoff, facts.clone()),
            (
                move || {
                    drop(write_output(&[(
                        Stream::Stderr,
                        &format!("Control API ready: {address}\n"),
                    )]));
                },
                stopped(shutdown.clone()),
            ),
        ));
        start_platform_roles(
            &mut tasks,
            (&store, &shutdown),
            RetentionPolicies {
                audit: facts.audit,
                statistics: facts.statistics,
                snapshots: facts.snapshots,
                run_logs: facts.run_logs,
            },
        )?;
    }
    if facts.scheduler {
        start_scheduler(&mut tasks, store, shutdown, workers);
    }
    supervise_control_roles(tasks, stop).await
}

fn start_scheduler(
    tasks: &mut JoinSet<Result<(), ControlError>>,
    store: ControlStore,
    shutdown: watch::Receiver<bool>,
    workers: Workers,
) {
    drop(write_output(&[(Stream::Stderr, "Starting scheduler...\n")]));
    tasks.spawn(async move {
        run_scheduler(
            &store,
            SchedulerState::new(Uuid::now_v7()),
            shutdown,
            workers,
        )
        .await
    });
    drop(write_output(&[(
        Stream::Stderr,
        "Scheduler loop started; leadership is lease-guarded\n",
    )]));
}

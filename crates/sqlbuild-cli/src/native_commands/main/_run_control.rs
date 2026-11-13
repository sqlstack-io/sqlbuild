//! Supervise native API/scheduler roles together; a failed role stops its siblings.

use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::watch;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{control_address, control_api_settings};
use crate::native_commands::_helpers::execution::{
    open_control_store, open_control_workers, start_platform_roles, supervise_control_roles,
};
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
    let (authentication, facts) = control_api_settings(namespace, standalone)?;
    let store = open_control_store(namespace).await?;
    let (workers, handoff) = open_control_workers(namespace)?;
    let (stop, shutdown) = watch::channel(false);
    let mut tasks = tokio::task::JoinSet::new();
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
            (
                facts.audit,
                facts.statistics,
                facts.snapshots,
                facts.run_logs,
            ),
        )?;
    }
    if facts.scheduler {
        drop(write_output(&[(Stream::Stderr, "Starting scheduler...\n")]));
        let scheduler_store = store.clone();
        tasks.spawn(async move {
            run_scheduler(
                &scheduler_store,
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
    supervise_control_roles(tasks, stop).await
}

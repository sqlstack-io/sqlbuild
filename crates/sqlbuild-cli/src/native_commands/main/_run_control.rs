//! Supervise native API/scheduler roles together; a failed role stops its siblings.

use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::sync::watch;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::command::decision::{
    control_address, control_api_settings, control_output,
};
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::{
    open_control_store, open_control_workers, start_api,
};
use crate::native_commands::_helpers::location::shutdown::supervise_control_roles;
use crate::server_config::main::read_server_config::read_server_config;
use crate::workers::models::Workers;

pub(crate) async fn run_control(
    namespace: &Namespace,
    standalone: bool,
) -> Result<(), ControlError> {
    let config = read_server_config(namespace)?;
    let address = control_address(namespace, standalone, &config.server)?;
    let (authentication, mut facts) = control_api_settings(namespace, standalone, &config)?;
    let store = open_control_store(namespace, Some(&config.server)).await?;
    let run_output = control_output(namespace, &config, (&store, &mut facts))?;
    let (workers, handoff) = open_control_workers(
        (&store, namespace),
        (&config.server, config.launchers),
        facts.scheduler,
    )
    .await?;
    facts.pool_caps = workers.pool_caps();
    let (stop, shutdown) = watch::channel(false);
    let mut tasks = JoinSet::new();
    if let Some(address) = address {
        tasks = start_api(
            tasks,
            (address, &store, &shutdown),
            (authentication, handoff, &facts),
            run_output,
        )
        .await?;
    }
    if facts.scheduler {
        start_scheduler(&mut tasks, (store, facts.origin.clone()), shutdown, workers);
    }
    supervise_control_roles(tasks, stop).await
}

/// Start the scheduler, whose sensor events link runs under `origin`.
fn start_scheduler(
    tasks: &mut JoinSet<Result<(), ControlError>>,
    (store, origin): (ControlStore, Option<String>),
    shutdown: watch::Receiver<bool>,
    workers: Workers,
) {
    drop(write_output(&[(Stream::Stderr, "Starting scheduler...\n")]));
    tasks.spawn(async move {
        run_scheduler(
            &store,
            SchedulerState::new(Uuid::now_v7()).with_origin(origin),
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

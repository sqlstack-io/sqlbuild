//! Start the control API and the platform roles that run beside it.

use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::api::models::{ApiAuthentication, ServerFacts};
use sqlbuild_orchestration::platform::models::RetentionPolicies;
use sqlbuild_orchestration::run_output::models::OutputStore;
use sqlbuild_orchestration::runs::types::RunHandoff;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::start_platform_roles;
use crate::native_commands::models::ApiStandby;

/// Resolve once `shutdown` turns true or its sender is gone.
async fn stopped(mut shutdown: watch::Receiver<bool>) {
    while !*shutdown.borrow_and_update() {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

/// Start the control API and the platform roles beside it, with the retention `facts` and `run_output` set.
pub(crate) async fn start_api(
    mut tasks: JoinSet<Result<(), ControlError>>,
    (standby, store, shutdown): (ApiStandby, &ControlStore, &watch::Receiver<bool>),
    (authentication, handoff, facts): (ApiAuthentication, RunHandoff, &ServerFacts),
    run_output: Option<(OutputStore, u16)>,
) -> Result<JoinSet<Result<(), ControlError>>, ControlError> {
    let ApiStandby { listener, end } = standby;
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
    // The API accepts on the same socket from here; the standby finishes what it has and stops.
    end.send(()).ok();
    start_platform_roles(
        &mut tasks,
        (store, shutdown),
        RetentionPolicies {
            audit: facts.audit,
            statistics: facts.statistics,
            snapshots: facts.snapshots,
            run_logs: facts.run_logs,
            state_history: facts.state_history,
            run_output,
        },
    )?;
    Ok(tasks)
}

//! A stop with runs in flight cancels them first, through the workers' own SIGTERM path, so each ends Cancelled with its statements cancelled and its claims released.

use std::time::Instant;

use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::runs::main::execution::change_run::change_run;
use sqlbuild_orchestration::runs::main::execution::observe_run::observe_run;
use sqlbuild_orchestration::runs::main::listings::list_runs::list_runs;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunKind, RunRecord, RunStatus,
};
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::dev_screen::models::ScreenMessage;
use crate::dev_stop::constants::{
    CANCEL_DEADLINE, CANCEL_POLL, CANCEL_REASON, DEV_ACTOR, DEV_GRANTING_ROLE, RUN_SCAN,
    STOP_DEADLINE, STOP_GRACE,
};
use crate::dev_stop::main::stop_switch::stop_switch;

/// The dev target whose runs a stop cancels.
pub(crate) struct DevRuns {
    pub(crate) store: ControlStore,
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
}

/// A run being cancelled: its ID, its name on the screen and the statements it was running.
struct Cancelling {
    run_id: Uuid,
    name: String,
    queries: String,
}

/// Ask every executing run to cancel, then wait for each to end, at most `CANCEL_DEADLINE`; whatever is left is killed, and the rest of the stop gets the usual deadline.
pub(crate) async fn cancel_active_runs(runs: &DevRuns, messages: &UnboundedSender<ScreenMessage>) {
    let active: Vec<RunRecord> =
        match list_runs(&runs.store, (runs.location_id, &runs.target), RUN_SCAN).await {
            Ok(records) => records.into_iter().filter(executing).collect(),
            Err(error) => {
                drop(messages.send(ScreenMessage::Log(format!(
                    "could not read the runs to cancel: {error}"
                ))));
                return;
            }
        };
    if active.is_empty() {
        return;
    }
    stop_switch().arm(CANCEL_DEADLINE + STOP_GRACE + STOP_DEADLINE);
    let began: Instant = Instant::now();
    let context: AuditContext = AuditContext {
        actor: DEV_ACTOR.to_owned(),
        granting_role: DEV_GRANTING_ROLE.to_owned(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    let mut pending: Vec<Cancelling> = Vec::new();
    for run in active {
        let cancelling: Cancelling = Cancelling {
            run_id: run.run_id,
            name: run.job_key.clone().unwrap_or_else(|| "ad hoc".to_owned()),
            queries: queries(run.node_counts_json.get("running").copied().unwrap_or(0)),
        };
        drop(messages.send(ScreenMessage::Cancelling {
            label: format!(
                "Cancelling run {}\u{2026} {} (Ctrl+C again to force)",
                cancelling.name, cancelling.queries
            ),
        }));
        let requested = change_run(
            &runs.store,
            RunAuthority::Operator(&context),
            &RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: run.run_id,
                claim_id: None,
                change: RunChange::CancelRequested {
                    actor: context.actor.clone(),
                    reason: CANCEL_REASON.to_owned(),
                },
            },
        )
        .await;
        if let Err(error) = requested {
            drop(messages.send(ScreenMessage::Log(format!(
                "could not cancel run {}: {error}",
                cancelling.name
            ))));
        }
        pending.push(cancelling);
    }
    while !pending.is_empty() && began.elapsed() < CANCEL_DEADLINE {
        tokio::time::sleep(CANCEL_POLL).await;
        let mut left: Vec<Cancelling> = Vec::new();
        for run in pending {
            match observe_run(&runs.store, run.run_id).await {
                Ok(state) if !open(state.status) => {
                    drop(messages.send(ScreenMessage::Stopped {
                        label: format!("Cancelled run {}  {} cancelled", run.name, run.queries),
                        elapsed: began.elapsed(),
                    }));
                }
                Ok(_) | Err(_) => left.push(run),
            }
        }
        pending = left;
    }
    if !pending.is_empty() {
        stop_switch().kill_all();
    }
    stop_switch().arm(STOP_GRACE + STOP_DEADLINE);
}

/// A run a worker is executing; queued runs stay queued, and a group ends with its members.
fn executing(run: &RunRecord) -> bool {
    run.kind != RunKind::Group
        && matches!(
            run.status,
            RunStatus::Launched | RunStatus::Running | RunStatus::Unresponsive
        )
}

/// Whether a run has not ended yet.
fn open(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Queued | RunStatus::Launched | RunStatus::Running | RunStatus::Unresponsive
    )
}

/// `1 query` or `N queries`.
fn queries(count: i64) -> String {
    match count {
        1 => "1 query".to_owned(),
        count => format!("{count} queries"),
    }
}

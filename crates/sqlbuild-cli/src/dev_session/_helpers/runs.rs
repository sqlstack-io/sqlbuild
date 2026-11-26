//! Event lines for runs: one when a run starts, one when it ends.

use std::collections::HashMap;
use std::time::Duration;

use sqlbuild_orchestration::runs::models::{RunOrigin, RunRecord, RunStatus};
use uuid::Uuid;

use crate::dev_screen::models::ScreenMessage;

/// The runs seen so far and their last status.
pub(crate) type SeenRuns = HashMap<Uuid, RunStatus>;

/// Messages for runs that started or ended since `seen`, and the runs now seen; the first read only records.
pub(crate) fn run_messages(
    seen: Option<SeenRuns>,
    runs: &[RunRecord],
) -> (Vec<ScreenMessage>, SeenRuns) {
    let now: SeenRuns = runs.iter().map(|run| (run.run_id, run.status)).collect();
    let Some(seen) = seen else {
        return (Vec::new(), now);
    };
    let mut messages: Vec<ScreenMessage> = Vec::new();
    for run in runs.iter().rev() {
        let before: Option<RunStatus> = seen.get(&run.run_id).copied();
        if before.is_none() {
            messages.push(ScreenMessage::Run {
                name: run_name(run),
                detail: started_by(run),
                finished: None,
            });
        }
        if run.status.is_terminal() && before.is_none_or(|status| !status.is_terminal()) {
            messages.push(ScreenMessage::Run {
                name: run_name(run),
                detail: format!(
                    "{} \u{b7} {} nodes",
                    outcome(run.status),
                    run.node_counts_json.values().sum::<i64>()
                ),
                finished: Some((
                    matches!(run.status, RunStatus::Success | RunStatus::Warn),
                    run_elapsed(run),
                )),
            });
        }
    }
    (messages, now)
}

/// How a finished run ended, in words.
fn outcome(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Success => "succeeded",
        RunStatus::Warn => "succeeded with warnings",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
        RunStatus::Skipped => "skipped",
        RunStatus::Queued | RunStatus::Launched | RunStatus::Running | RunStatus::Unresponsive => {
            "ended"
        }
    }
}

/// `started manually`, `started by schedule nightly` or `started as a rerun`; the trigger is named unless it already names the run.
fn started_by(run: &RunRecord) -> String {
    let trigger: String = match (&run.job_key, &run.trigger_name) {
        (Some(_), Some(name)) => format!(" {name}"),
        _ => String::new(),
    };
    match run.origin {
        RunOrigin::Manual => "started manually".to_owned(),
        RunOrigin::Schedule => format!("started by schedule{trigger}"),
        RunOrigin::Sensor => format!("started by sensor{trigger}"),
        RunOrigin::Rerun => "started as a rerun".to_owned(),
        RunOrigin::Backfill => "started by a backfill".to_owned(),
    }
}

fn run_name(run: &RunRecord) -> String {
    run.job_key
        .clone()
        .or_else(|| run.trigger_name.clone())
        .unwrap_or_else(|| run.kind.as_str().to_owned())
}

fn run_elapsed(run: &RunRecord) -> Duration {
    match (run.started_at, run.ended_at) {
        (Some(started), Some(ended)) => (ended - started).to_std().unwrap_or_default(),
        _ => Duration::ZERO,
    }
}

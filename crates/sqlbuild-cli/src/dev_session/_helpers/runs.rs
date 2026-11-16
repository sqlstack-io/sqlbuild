//! Event lines for runs: one when a run starts, one when it ends.

use std::collections::HashMap;
use std::time::Duration;

use sqlbuild_orchestration::runs::models::{RunRecord, RunStatus};
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
                detail: format!("started from {}", run.origin.as_str()),
                finished: None,
            });
        }
        if run.status.is_terminal() && before.is_none_or(|status| !status.is_terminal()) {
            messages.push(ScreenMessage::Run {
                name: run_name(run),
                detail: format!("{} nodes", run.node_counts_json.values().sum::<i64>()),
                finished: Some((
                    matches!(run.status, RunStatus::Success | RunStatus::Warn),
                    run_elapsed(run),
                )),
            });
        }
    }
    (messages, now)
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

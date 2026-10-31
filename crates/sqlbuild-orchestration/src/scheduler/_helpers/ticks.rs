//! Stable IDs and dispositions, shared by catch-up and ordinary schedule evaluation.

use chrono::{DateTime, SecondsFormat, Utc};
use uuid::Uuid;

use crate::scheduler::models::{CatchUp, Overlap, TickAction, TickDecision, TickState};

pub(crate) fn catch_up_actions(state: &TickState, due: &[DateTime<Utc>]) -> Vec<TickAction> {
    let covering_run = (state.overlap == Overlap::Coalesce)
        .then_some(state.waiting_run)
        .flatten();
    match (covering_run, state.catch_up) {
        (Some(run_id), _) => due
            .iter()
            .map(|time| action(state.trigger_id, *time, Some(run_id)))
            .collect(),
        (None, CatchUp::All { max_runs }) => due
            .iter()
            .take(max_runs.get())
            .map(|time| action(state.trigger_id, *time, None))
            .collect(),
        (None, CatchUp::Latest) => {
            let latest = due.last().copied();
            let run_id = latest.map(|time| requested_run(tick_id(state.trigger_id, time)));
            due.iter()
                .map(|time| {
                    action(
                        state.trigger_id,
                        *time,
                        (Some(*time) != latest).then_some(run_id).flatten(),
                    )
                })
                .collect()
        }
    }
}

pub(crate) fn tick_id(trigger_id: Uuid, scheduled_for: DateTime<Utc>) -> Uuid {
    Uuid::new_v5(
        &trigger_id,
        scheduled_for
            .to_rfc3339_opts(SecondsFormat::Nanos, true)
            .as_bytes(),
    )
}

pub(crate) fn requested_run(tick_id: Uuid) -> Uuid {
    Uuid::new_v5(&tick_id, b"run")
}

pub(crate) fn action(
    trigger_id: Uuid,
    scheduled_for: DateTime<Utc>,
    covering_run: Option<Uuid>,
) -> TickAction {
    let tick_id = tick_id(trigger_id, scheduled_for);
    let decision = match covering_run {
        Some(covered_by_run_id) => TickDecision::Covered { covered_by_run_id },
        None => TickDecision::Requested {
            run_id: requested_run(tick_id),
        },
    };
    TickAction {
        tick_id,
        scheduled_for,
        decision,
    }
}

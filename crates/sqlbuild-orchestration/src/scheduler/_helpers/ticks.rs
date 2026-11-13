//! Stable tick and run IDs, so a retried evaluation reproduces the same decision.

use chrono::{DateTime, SecondsFormat, Utc};
use uuid::Uuid;

use crate::scheduler::models::{TickAction, TickDecision};

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

/// A waiting run covers the tick before an active one skips it.
pub(crate) fn action(
    trigger_id: Uuid,
    scheduled_for: DateTime<Utc>,
    (covering_run, active_run): (Option<Uuid>, Option<Uuid>),
) -> TickAction {
    let tick_id = tick_id(trigger_id, scheduled_for);
    let decision = match (covering_run, active_run) {
        (Some(covered_by_run_id), _) => TickDecision::Covered { covered_by_run_id },
        (None, Some(active_run_id)) => TickDecision::Skipped { active_run_id },
        (None, None) => TickDecision::Requested {
            run_id: requested_run(tick_id),
        },
    };
    TickAction {
        tick_id,
        scheduled_for,
        decision,
    }
}

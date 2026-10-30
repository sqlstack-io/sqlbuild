//! Stable IDs and dispositions, shared by catch-up and ordinary schedule evaluation.

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

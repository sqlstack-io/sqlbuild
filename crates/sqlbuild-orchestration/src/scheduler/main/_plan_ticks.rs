//! Pure scheduling decisions: crashes and clock jumps cannot change a tick's identity.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};

use crate::scheduler::_helpers::ticks::catch_up_actions;
use crate::scheduler::constants::MAX_TICKS_PER_BATCH;
use crate::scheduler::models::{TickPlan, TickState};
use crate::store::errors::ControlError;

/// Return facts to append and the cursor to commit atomically with them.
pub(in crate::scheduler) fn plan_ticks(
    state: &TickState,
    now: DateTime<Utc>,
) -> Result<TickPlan, ControlError> {
    if !state.enabled {
        return Ok(TickPlan {
            actions: vec![],
            last_tick_for: state.last_tick_for,
            next_due_at: None,
        });
    }
    if state.due_times.len() > MAX_TICKS_PER_BATCH {
        return Err(ControlError::Invalid(
            "schedule expansion exceeds the bounded tick batch".into(),
        ));
    }
    let pending: BTreeSet<DateTime<Utc>> = state
        .due_times
        .iter()
        .copied()
        .filter(|time| state.last_tick_for.is_none_or(|last| *time > last))
        .collect();
    let due: Vec<DateTime<Utc>> = pending
        .iter()
        .copied()
        .take_while(|time| *time <= now)
        .collect();
    let actions = catch_up_actions(state, &due);
    let last_tick_for = actions
        .iter()
        .rev()
        .find(|action| {
            state
                .continuation
                .is_none_or(|next| action.scheduled_for < next)
        })
        .map(|action| action.scheduled_for)
        .or(state.last_tick_for);
    let next_due_at = pending
        .into_iter()
        .chain(state.continuation)
        .filter(|time| last_tick_for.is_none_or(|last| *time > last))
        .min();
    Ok(TickPlan {
        actions,
        last_tick_for,
        next_due_at,
    })
}

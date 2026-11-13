//! Pure scheduling decisions: crashes and clock jumps cannot change a tick's identity.

use chrono::{DateTime, TimeDelta, Utc};

use crate::declarations::models::TriggerDefinition;
use crate::scheduler::_helpers::expansion::latest_occurrence;
use crate::scheduler::_helpers::ticks::action;
use crate::scheduler::main::next_schedule_time::next_schedule_time;
use crate::scheduler::models::{TickPlan, TickState};
use crate::store::errors::ControlError;

/// Only the latest missed occurrence ticks; a waiting unclaimed run covers it, and an active run skips it unless earlier occurrences were missed.
pub(in crate::scheduler) fn plan_ticks(
    trigger: &TriggerDefinition,
    state: &TickState,
    now: DateTime<Utc>,
) -> Result<TickPlan, ControlError> {
    let after = match state.last_tick_for {
        Some(last) => last,
        None => state
            .started_at
            .checked_sub_signed(TimeDelta::microseconds(1))
            .ok_or_else(|| ControlError::Invalid("trigger start is out of range".into()))?,
    };
    let latest = latest_occurrence(trigger, now)?.filter(|time| *time > after);
    let catching_up = match latest {
        Some(time) => {
            next_schedule_time(&trigger.schedule, after)?.is_some_and(|first| first < time)
        }
        None => false,
    };
    let active_run = state.active_run.filter(|_| !catching_up);
    Ok(TickPlan {
        actions: latest
            .map(|time| action(state.trigger_id, time, (state.waiting_run, active_run)))
            .into_iter()
            .collect(),
        last_tick_for: latest.or(state.last_tick_for),
        next_due_at: next_schedule_time(&trigger.schedule, latest.unwrap_or(after))?,
    })
}

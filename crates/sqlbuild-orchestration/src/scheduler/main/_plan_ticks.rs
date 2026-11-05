//! Pure scheduling decisions: crashes and clock jumps cannot change a tick's identity.

use chrono::{DateTime, TimeDelta, Utc};

use crate::declarations::models::TriggerDefinition;
use crate::scheduler::_helpers::expansion::latest_occurrence;
use crate::scheduler::_helpers::ticks::action;
use crate::scheduler::main::next_schedule_time::next_schedule_time;
use crate::scheduler::models::{TickPlan, TickState};
use crate::store::errors::ControlError;

/// Only the latest missed occurrence ticks; a waiting unclaimed run covers it instead of queueing another.
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
    Ok(TickPlan {
        actions: latest
            .map(|time| action(state.trigger_id, time, state.waiting_run))
            .into_iter()
            .collect(),
        last_tick_for: latest.or(state.last_tick_for),
        next_due_at: next_schedule_time(&trigger.schedule, latest.unwrap_or(after))?,
    })
}

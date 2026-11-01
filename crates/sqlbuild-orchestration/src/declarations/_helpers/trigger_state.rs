//! Recompute the effective schedule after code publication or an operator override.

use chrono::{DateTime, TimeDelta, Utc};

use crate::declarations::models::{
    PreviousTriggerState, Schedule, TriggerDefinition, TriggerStatus,
};
use crate::scheduler::constants::MAX_TICKS_PER_BATCH;
use crate::scheduler::main::next_schedule_time::next_schedule_time;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;

/// Replacing a future horizon must retain due ticks not yet consumed by the scheduler.
pub(crate) fn retain_calendar_ticks(
    name: &str,
    trigger: &mut TriggerDefinition,
    state: Option<&PreviousTriggerState>,
    snapshot: &SnapshotRecord,
) -> Result<(), ControlError> {
    if let Some(state) = state
        && let Schedule::Calendar {
            times: previous_times,
            ..
        } = &state.definition_json.schedule
        && let Schedule::Calendar { times, .. } = &mut trigger.schedule
    {
        times.extend(
            previous_times
                .iter()
                .filter(|time| {
                    **time >= state.started_at
                        && **time <= snapshot.published_at
                        && state.last_tick_for.is_none_or(|last| **time > last)
                })
                .copied(),
        );
        if times.len() > MAX_TICKS_PER_BATCH {
            return Err(ControlError::Invalid(format!(
                "calendar {name} pending and upcoming times exceed {MAX_TICKS_PER_BATCH}; let the scheduler drain pending ticks before refreshing"
            )));
        }
    }
    Ok(())
}

pub(crate) fn trigger_timing(
    definition: &TriggerDefinition,
    started_at: DateTime<Utc>,
    last_tick_for: Option<DateTime<Utc>>,
    status_override: Option<TriggerStatus>,
) -> Result<(TriggerStatus, Option<DateTime<Utc>>), ControlError> {
    let status = status_override.unwrap_or(definition.default_status);
    let after = last_tick_for.unwrap_or(
        started_at
            .checked_sub_signed(TimeDelta::microseconds(1))
            .ok_or_else(|| ControlError::Invalid("trigger start is out of range".into()))?,
    );
    let next = if status == TriggerStatus::Running {
        next_schedule_time(&definition.schedule, after)?
    } else {
        None
    };
    Ok((status, next))
}

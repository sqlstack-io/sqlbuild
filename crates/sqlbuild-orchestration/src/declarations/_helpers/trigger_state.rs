//! Recompute the effective schedule after code publication or an operator override.

use chrono::{DateTime, TimeDelta, Utc};

use crate::declarations::constants::UPCOMING_TIMES;
use crate::declarations::models::{
    PreviousTriggerState, Schedule, StoredCalendarTimes, StoredSchedule, StoredTriggerState,
    TriggerDeclaration, TriggerDefinition, TriggerStatus,
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
        && let Some(Schedule::Calendar {
            times: previous_times,
            ..
        }) = state
            .definition_json
            .as_ref()
            .map(|definition| &definition.schedule)
        && let Schedule::Calendar { times, .. } = &mut trigger.schedule
    {
        times.extend(
            previous_times
                .iter()
                .filter(|time| pending(**time, state, snapshot))
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

/// The state before a republish, and the pending calendar times it drops: those of a previous calendar whose definition no longer decodes, while `trigger` is a calendar too. Its timing columns, operator status and cursor survive either way.
pub(crate) fn previous_state(
    stored: StoredTriggerState,
    trigger: &TriggerDefinition,
    snapshot: &SnapshotRecord,
) -> (PreviousTriggerState, Vec<DateTime<Utc>>) {
    let (definition_json, times) =
        match serde_json::from_value::<TriggerDefinition>(stored.definition_json.clone()) {
            Ok(definition) => (Some(definition), Vec::new()),
            Err(_) => (None, stored_calendar_times(stored.definition_json, trigger)),
        };
    let state = PreviousTriggerState {
        definition_json,
        started_at: stored.started_at,
        last_tick_for: stored.last_tick_for,
        status_override: stored.status_override,
    };
    let dropped: Vec<DateTime<Utc>> = times
        .into_iter()
        .filter(|time| pending(*time, &state, snapshot))
        .collect();
    (state, dropped)
}

/// The calendar times of an undecodable stored definition when it and `trigger` are both calendars; otherwise none, since nothing would have been retained.
fn stored_calendar_times(
    stored: serde_json::Value,
    trigger: &TriggerDefinition,
) -> Vec<DateTime<Utc>> {
    let times: Vec<DateTime<Utc>> = match serde_json::from_value::<StoredCalendarTimes>(stored) {
        Ok(StoredCalendarTimes {
            schedule: StoredSchedule::Calendar { times },
        }) => times,
        Ok(StoredCalendarTimes {
            schedule: StoredSchedule::Other,
        })
        | Err(_) => Vec::new(),
    };
    if matches!(trigger.schedule, Schedule::Calendar { .. }) {
        times
    } else {
        Vec::new()
    }
}

/// A previous calendar time that was due by the publish but not yet ticked.
fn pending(time: DateTime<Utc>, state: &PreviousTriggerState, snapshot: &SnapshotRecord) -> bool {
    time >= state.started_at
        && time <= snapshot.published_at
        && state.last_tick_for.is_none_or(|last| time > last)
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

/// The next due times of a running schedule from `next_due_at` on, as the scheduler works them out.
pub(crate) fn upcoming_times(
    declaration: &TriggerDeclaration,
    next_due_at: Option<DateTime<Utc>>,
) -> Result<Vec<DateTime<Utc>>, ControlError> {
    let (TriggerDeclaration::Schedule(definition), Some(first)) = (declaration, next_due_at) else {
        return Ok(Vec::new());
    };
    let mut times: Vec<DateTime<Utc>> = vec![first];
    while times.len() < UPCOMING_TIMES {
        let Some(next) = next_schedule_time(&definition.schedule, times[times.len() - 1])? else {
            break;
        };
        times.push(next);
    }
    Ok(times)
}

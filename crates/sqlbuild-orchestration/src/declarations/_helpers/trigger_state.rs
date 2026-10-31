//! Recompute the effective schedule after code publication or an operator override.

use chrono::{DateTime, TimeDelta, Utc};

use crate::declarations::models::{TriggerDefinition, TriggerStatus};
use crate::scheduler::main::next_schedule_time::next_schedule_time;
use crate::store::errors::ControlError;

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

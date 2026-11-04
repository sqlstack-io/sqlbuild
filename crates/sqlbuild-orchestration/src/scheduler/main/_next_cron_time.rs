//! Calculate the first distinct UTC tick after a persisted cursor.

use chrono::{DateTime, Timelike, Utc};

use crate::scheduler::_helpers::calendar::{resolve_wall_time, search_start};
use crate::scheduler::models::CronSchedule;
use crate::store::errors::ControlError;

/// Later fold for ambiguities, IANA gap end for missing times, searching from the cursor's whole second.
pub(in crate::scheduler) fn next_cron_time(
    schedule: &CronSchedule,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, ControlError> {
    let mut cursor = search_start(schedule.timezone, after)
        .with_nanosecond(0)
        .ok_or_else(|| ControlError::Invalid("cron cursor is out of range".into()))?
        .and_utc();
    loop {
        let candidate = schedule
            .expression
            .find_next_occurrence(&cursor, false)
            .map_err(|error| {
                ControlError::Invalid(format!("cron has no next occurrence: {error}"))
            })?;
        let time = resolve_wall_time(schedule.timezone, candidate.naive_utc())?;
        if time > after {
            return Ok(time);
        }
        cursor = candidate;
    }
}

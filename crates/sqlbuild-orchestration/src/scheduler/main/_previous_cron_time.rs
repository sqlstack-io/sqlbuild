//! Locate the latest due tick without expanding an unbounded catch-up backlog.

use chrono::{DateTime, Timelike, Utc};

use crate::scheduler::_helpers::calendar::resolve_wall_time;
use crate::scheduler::models::CronSchedule;
use crate::store::errors::ControlError;

/// Forward expansion's fold and gap mapping, searching from `at`'s whole second so results stay on occurrences.
pub(in crate::scheduler) fn previous_cron_time(
    schedule: &CronSchedule,
    at: DateTime<Utc>,
) -> Result<DateTime<Utc>, ControlError> {
    let mut cursor = at
        .with_timezone(&schedule.timezone)
        .naive_local()
        .with_nanosecond(0)
        .ok_or_else(|| ControlError::Invalid("cron search time is out of range".into()))?
        .and_utc();
    let mut inclusive = true;
    loop {
        let candidate = schedule
            .expression
            .find_previous_occurrence(&cursor, inclusive)
            .map_err(|error| {
                ControlError::Invalid(format!("cron has no previous occurrence: {error}"))
            })?;
        let time = resolve_wall_time(schedule.timezone, candidate.naive_utc())?;
        if time <= at {
            return Ok(time);
        }
        cursor = candidate;
        inclusive = false;
    }
}

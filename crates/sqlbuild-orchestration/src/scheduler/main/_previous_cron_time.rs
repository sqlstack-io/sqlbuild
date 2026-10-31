//! Locate the latest due tick without expanding an unbounded catch-up backlog.

use chrono::{DateTime, Utc};

use crate::scheduler::_helpers::calendar::resolve_wall_time;
use crate::scheduler::models::CronSchedule;
use crate::store::errors::ControlError;

/// Fold and gap mapping is identical to forward expansion, including the later ambiguous instant.
pub(in crate::scheduler) fn previous_cron_time(
    schedule: &CronSchedule,
    at: DateTime<Utc>,
) -> Result<DateTime<Utc>, ControlError> {
    let mut cursor = at.with_timezone(&schedule.timezone).naive_local().and_utc();
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

//! Find the next due time for a resolved cron or calendar declaration.

use std::ops::Bound::{Excluded, Unbounded};

use chrono::{DateTime, Utc};

use crate::declarations::models::Schedule;
use crate::scheduler::main::_next_cron_time::next_cron_time;
use crate::scheduler::main::parse_cron::parse_cron;
use crate::store::errors::ControlError;

pub(crate) fn next_schedule_time(
    schedule: &Schedule,
    after: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, ControlError> {
    match schedule {
        Schedule::Cron {
            expression,
            timezone,
        } => Ok(Some(next_cron_time(
            &parse_cron(expression, timezone)?,
            after,
        )?)),
        Schedule::Calendar { times, .. } => {
            Ok(times.range((Excluded(after), Unbounded)).next().copied())
        }
    }
}

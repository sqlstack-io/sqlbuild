//! Find the newest due occurrence without expanding the history before it.

use std::ops::Bound::{Included, Unbounded};

use chrono::{DateTime, Utc};

use crate::declarations::models::{Schedule, TriggerDefinition};
use crate::scheduler::main::_previous_cron_time::previous_cron_time;
use crate::scheduler::main::parse_cron::parse_cron;
use crate::store::errors::ControlError;

/// The newest occurrence at or before `now`.
pub(crate) fn latest_occurrence(
    trigger: &TriggerDefinition,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, ControlError> {
    match &trigger.schedule {
        Schedule::Cron {
            expression,
            timezone,
        } => previous_cron_time(&parse_cron(expression, timezone)?, now).map(Some),
        Schedule::Calendar { times, .. } => {
            Ok(times.range((Unbounded, Included(now))).next_back().copied())
        }
    }
}

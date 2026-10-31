//! Bound cron backlog work without losing omitted times or requesting an obsolete latest run.

use std::ops::Bound::{Excluded, Unbounded};

use chrono::{DateTime, Utc};

use crate::declarations::models::{Schedule, TriggerDefinition};
use crate::scheduler::constants::MAX_TICKS_PER_BATCH;
use crate::scheduler::main::_next_cron_time::next_cron_time;
use crate::scheduler::main::_previous_cron_time::previous_cron_time;
use crate::scheduler::main::parse_cron::parse_cron;
use crate::scheduler::models::CatchUp;
use crate::scheduler::types::ScheduleExpansion;
use crate::store::errors::ControlError;

pub(crate) fn expand_schedule(
    trigger: &TriggerDefinition,
    from: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<ScheduleExpansion, ControlError> {
    let (expression, timezone) = match &trigger.schedule {
        Schedule::Cron {
            expression,
            timezone,
        } => (expression, timezone),
        Schedule::Calendar { times, .. } => {
            return Ok((
                times.range((Excluded(from), Unbounded)).copied().collect(),
                None,
            ));
        }
    };
    let cron = parse_cron(expression, timezone)?;
    let latest = trigger.catch_up == CatchUp::Latest;
    let mut due: Vec<DateTime<Utc>> = Vec::new();
    let mut cursor = from;
    for _ in 0..(MAX_TICKS_PER_BATCH - usize::from(latest)) {
        cursor = next_cron_time(&cron, cursor)?;
        due.push(cursor);
        if cursor > now {
            return Ok((due, None));
        }
    }
    let continuation = next_cron_time(&cron, cursor)?;
    if latest && continuation <= now {
        let latest = previous_cron_time(&cron, now)?;
        due.push(latest);
        return Ok((
            due,
            Some(if latest == continuation {
                next_cron_time(&cron, latest)?
            } else {
                continuation
            }),
        ));
    }
    Ok((due, Some(continuation)))
}

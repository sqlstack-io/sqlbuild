//! Validate POSIX minute-resolution schedules and IANA time zones before publication.

use croner::parser::{CronParser, Seconds, Year};

use crate::scheduler::models::CronSchedule;
use crate::store::errors::ControlError;

/// Day-of-month and day-of-week use POSIX OR, not Quartz AND semantics.
pub fn parse_cron(expression: &str, timezone: &str) -> Result<CronSchedule, ControlError> {
    let expression = CronParser::builder()
        .seconds(Seconds::Disallowed)
        .year(Year::Disallowed)
        .dom_and_dow(false)
        .build()
        .parse(expression)
        .map_err(|error| ControlError::Invalid(format!("invalid cron expression: {error}")))?;
    let timezone = timezone
        .parse()
        .map_err(|error| ControlError::Invalid(format!("invalid IANA time zone: {error}")))?;
    Ok(CronSchedule {
        expression,
        timezone,
    })
}

//! Native cost request validation and history bounds.

use std::time::{SystemTime, UNIX_EPOCH};

use sqlbuild_core::cost_command::constants::{
    DEFAULT_DETAIL_SORT, DEFAULT_HISTORY_SORT, HISTORY_SELECTOR, ISO_DATE_LENGTH,
};
use sqlbuild_core::cost_command::errors::CostCommandError;
use sqlbuild_core::cost_command::main::is_zero_cost_duration::is_zero_cost_duration;
use sqlbuild_core::cost_command::main::validate_cost_request::validate_cost_request;
use sqlbuild_core::cost_command::models::CostReportOptions;
use sqlbuild_cursor::algebra::main::parse_duration::parse_duration;
use sqlbuild_cursor::algebra::models::DurationDigits;
use sqlbuild_model_config::model_validation::main::parse_iso_timestamp::parse_iso_timestamp;

use crate::native_commands::_helpers::terminal::{stdout_supports_color, terminal_columns};
use crate::native_commands::constants::COST_ISO_PYTHON_MINOR;
use crate::native_commands::models::CostOptions;

fn bound(value: &str, label: &str, end_of_day: bool) -> Result<i128, CostCommandError> {
    let (instant, aware) =
        parse_iso_timestamp(&value.replace('Z', "+00:00"), COST_ISO_PYTHON_MINOR).ok_or_else(
            || {
                CostCommandError::new(
                    "C707",
                    format!("cost {label} must be an ISO date or datetime"),
                )
            },
        )?;
    if value.chars().count() == ISO_DATE_LENGTH {
        return Ok(i128::from(instant) + if end_of_day { 86_400_000_000 } else { 0 });
    }
    if !aware {
        return Err(CostCommandError::new(
            "C708",
            format!("cost {label} datetime must include a timezone"),
        ));
    }
    Ok(i128::from(instant))
}

fn since(value: &str) -> Result<i128, CostCommandError> {
    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_micros() as i128,
        Err(error) => -(error.duration().as_micros() as i128),
    };
    if is_zero_cost_duration(value) {
        return Ok(now);
    }
    let Some(DurationDigits {
        years,
        months,
        days,
        hours,
        minutes,
        seconds,
    }) = parse_duration(value)
    else {
        return bound(value, "--since", false);
    };
    let used = [&years, &months, &days, &hours, &minutes, &seconds]
        .into_iter()
        .filter(|value| !value.trim_start_matches('0').is_empty())
        .count();
    if used != 1
        || [&years, &months, &seconds]
            .into_iter()
            .any(|value| !value.trim_start_matches('0').is_empty())
    {
        return Err(CostCommandError::new(
            "C707",
            "cost --since relative duration must use one of: d, h, m",
        ));
    }
    let seconds = [(days, 86_400_i128), (hours, 3_600), (minutes, 60)]
        .into_iter()
        .try_fold(0_i128, |total, (digits, scale)| {
            let Ok(amount) = digits.parse::<i128>() else {
                return None;
            };
            total.checked_add(amount.checked_mul(scale)?)
        })
        .ok_or_else(|| {
            CostCommandError::new("C707", "cost --since relative duration is out of range")
        })?;
    seconds
        .checked_mul(1_000_000)
        .and_then(|micros| now.checked_sub(micros))
        .ok_or_else(|| {
            CostCommandError::new("C707", "cost --since relative duration is out of range")
        })
}

/// Resolve defaults and bounds once, before report selection or output side effects.
pub(super) fn cost_report_options(
    options: &CostOptions,
) -> Result<CostReportOptions, CostCommandError> {
    let history = options.selector == HISTORY_SELECTOR;
    let sort = options
        .sort
        .clone()
        .filter(|sort| !sort.is_empty())
        .unwrap_or_else(|| {
            if history {
                DEFAULT_HISTORY_SORT
            } else {
                DEFAULT_DETAIL_SORT
            }
            .to_owned()
        });
    validate_cost_request(
        &options.selector,
        (options.limit, options.no_limit),
        &sort,
        options.since.is_some() || options.until.is_some(),
    )?;
    Ok(CostReportOptions {
        selector: options.selector.clone(),
        limit: options.limit,
        no_limit: options.no_limit,
        sort,
        descending: options.descending,
        since: options.since.as_deref().map(since).transpose()?,
        until: options
            .until
            .as_deref()
            .map(|value| bound(value, "--until", true))
            .transpose()?,
        until_exclusive: options
            .until
            .as_deref()
            .is_some_and(|value| value.chars().count() == ISO_DATE_LENGTH),
        use_color: !options.no_color && stdout_supports_color(),
        terminal_width: usize::try_from(terminal_columns().unwrap_or(100)).unwrap_or(100),
    })
}

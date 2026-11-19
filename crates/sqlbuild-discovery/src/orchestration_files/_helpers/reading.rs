//! Positions, unknown keys and value checks shared by job and group files.

use std::collections::BTreeMap;

use sqlbuild_core::text::main::close_matches::close_matches;
use toml::Spanned;
use toml::de::DeTable;

use crate::orchestration_files::_helpers::raw::{RawRetry, RawRun, RawSchedule};
use crate::orchestration_files::constants::{
    ATTEMPTS_RANGE, CONCURRENCY_KEY, MAX_POOL_AMOUNT, REMOVED_RETRY_TABLE, RETRY_ATTEMPTS_KEY,
    RETRY_DELAY_KEY, RETRY_DELAY_RANGE, RETRY_TABLE, SUGGESTION_CUTOFF,
};
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{
    FilePosition, Positioned, RetryPolicy, RunFile, ScheduleFile, ScheduleSource,
};

/// The 1-based line and character column of byte `offset` in `text`.
pub(crate) fn position(text: &str, offset: usize) -> FilePosition {
    let before: &str = &text[..offset.min(text.len())];
    let line_start: usize = before.rfind('\n').map_or(0, |index| index + 1);
    FilePosition {
        line: before.matches('\n').count() + 1,
        column: before[line_start..].chars().count() + 1,
    }
}

/// An error at byte `offset`.
pub(crate) fn error_at(text: &str, offset: usize, message: String) -> FileError {
    FileError {
        message,
        position: position(text, offset),
        help: None,
    }
}

/// A TOML syntax or type error, at the span the parser reports.
pub(crate) fn toml_error(text: &str, error: &toml::de::Error) -> FileError {
    error_at(
        text,
        error.span().map_or(0, |span| span.start),
        error.message().trim_end().to_owned(),
    )
}

/// The value with the position its span starts at.
pub(crate) fn positioned<T>(text: &str, spanned: Spanned<T>) -> Positioned<T> {
    let start: usize = spanned.span().start;
    Positioned {
        value: spanned.into_inner(),
        position: position(text, start),
    }
}

/// Refuse a top-level key outside `allowed`, at the key, with the nearest known key as help.
pub(crate) fn check_keys(text: &str, allowed: &[&str], kind: &str) -> Result<(), FileError> {
    let table = DeTable::parse(text).map_err(|error| toml_error(text, &error))?;
    for key in table.get_ref().keys() {
        let name: &str = key.get_ref();
        if allowed.contains(&name) {
            continue;
        }
        let help: String = if name == CONCURRENCY_KEY {
            format!("move it under [settings] as `{CONCURRENCY_KEY} = ...`")
        } else if name == REMOVED_RETRY_TABLE.0 {
            REMOVED_RETRY_TABLE.1.to_owned()
        } else if let Some(nearest) = close_matches(name, allowed, 1, SUGGESTION_CUTOFF).first() {
            format!("did you mean '{nearest}'?")
        } else {
            let mut sorted: Vec<&str> = allowed.to_vec();
            sorted.sort_unstable();
            format!("supported keys: {}", sorted.join(", "))
        };
        return Err(FileError {
            help: Some(help),
            ..error_at(
                text,
                key.span().start,
                format!("unknown {kind} key '{name}'"),
            )
        });
    }
    Ok(())
}

/// A string that must hold more than whitespace.
pub(crate) fn nonblank(
    text: &str,
    value: Spanned<String>,
    what: &str,
) -> Result<Positioned<String>, FileError> {
    if value.get_ref().trim().is_empty() {
        return Err(error_at(
            text,
            value.span().start,
            format!("{what} must not be blank"),
        ));
    }
    Ok(positioned(text, value))
}

fn schedule(text: &str, raw: Spanned<RawSchedule>) -> Result<ScheduleFile, FileError> {
    let start: usize = raw.span().start;
    let raw: RawSchedule = raw.into_inner();
    let source: ScheduleSource = match (raw.cron, raw.calendar) {
        (Some(cron), None) => ScheduleSource::Cron(positioned(text, cron)),
        (None, Some(calendar)) => {
            ScheduleSource::Calendar(nonblank(text, calendar, "schedule calendar")?)
        }
        _ => {
            return Err(error_at(
                text,
                start,
                "schedule needs exactly one of `cron` or `calendar`".to_owned(),
            ));
        }
    };
    Ok(ScheduleFile {
        source,
        timezone: positioned(text, raw.timezone),
        starts_at: raw.starts_at.map(|starts_at| positioned(text, starts_at)),
        default_status: raw.default_status,
    })
}

/// One `[retry]` number within `range`, if set.
fn retry_number<T: TryFrom<i64>>(
    text: &str,
    (key, value): (&str, Option<Spanned<i64>>),
    range: (i64, i64),
) -> Result<Option<T>, FileError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let start: usize = value.span().start;
    let number: i64 = value.into_inner();
    let invalid = || {
        error_at(
            text,
            start,
            format!(
                "{RETRY_TABLE} {key} must be a whole number from {} to {}",
                range.0, range.1
            ),
        )
    };
    if !(range.0..=range.1).contains(&number) {
        return Err(invalid());
    }
    T::try_from(number).map(Some).map_err(|_| invalid())
}

/// `[retry]`, checked.
fn retry(text: &str, raw: Option<Spanned<RawRetry>>) -> Result<Option<RetryPolicy>, FileError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let start: usize = raw.span().start;
    let raw: RawRetry = raw.into_inner();
    let attempts: Option<u8> = retry_number(
        text,
        (RETRY_ATTEMPTS_KEY, raw.attempts),
        (i64::from(ATTEMPTS_RANGE.0), i64::from(ATTEMPTS_RANGE.1)),
    )?;
    let delay_seconds: Option<u32> = retry_number(
        text,
        (RETRY_DELAY_KEY, raw.delay_seconds),
        (
            i64::from(RETRY_DELAY_RANGE.0),
            i64::from(RETRY_DELAY_RANGE.1),
        ),
    )?;
    let attempts: u8 = attempts.ok_or_else(|| {
        error_at(
            text,
            start,
            format!("{RETRY_TABLE} needs `{RETRY_ATTEMPTS_KEY}`"),
        )
    })?;
    Ok(Some(RetryPolicy {
        attempts,
        delay_seconds: delay_seconds.unwrap_or(RETRY_DELAY_RANGE.0),
    }))
}

/// The fields jobs and groups share, checked.
pub(crate) fn run_file(text: &str, raw: RawRun) -> Result<RunFile, FileError> {
    let owner: Option<Positioned<String>> = raw
        .owner
        .map(|owner| nonblank(text, owner, "owner"))
        .transpose()?;
    let mut pools: BTreeMap<String, u32> = BTreeMap::new();
    for (pool, amount) in raw.pools {
        let slots: u32 = match u32::try_from(*amount.get_ref()) {
            Ok(slots) if (1..=MAX_POOL_AMOUNT).contains(&i64::from(slots)) => slots,
            _ => {
                return Err(error_at(
                    text,
                    amount.span().start,
                    format!(
                        "pools.{pool} must take a whole number of slots from 1 to \
                         {MAX_POOL_AMOUNT}, such as {pool} = 1"
                    ),
                ));
            }
        };
        pools.insert(pool, slots);
    }
    let name: Positioned<String> = nonblank(text, raw.name, "name")?;
    let description: String = nonblank(text, raw.description, "description")?.value;
    let scheduled: Option<ScheduleFile> = raw
        .schedule
        .map(|value| schedule(text, value))
        .transpose()?;
    let retry: Option<RetryPolicy> = retry(text, raw.retry)?;
    Ok(RunFile {
        name,
        description,
        targets: raw.targets,
        owner: owner.map(|owner| owner.value),
        overlap: raw.overlap,
        pools,
        schedule: scheduled,
        retry,
    })
}

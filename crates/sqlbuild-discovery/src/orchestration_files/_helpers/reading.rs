//! Positions, unknown keys and value checks shared by job and group files.

use std::collections::BTreeMap;

use sqlbuild_core::text::main::close_matches::close_matches;
use toml::Spanned;
use toml::de::DeTable;

use crate::orchestration_files::_helpers::raw::{RawRetry, RawRun, RawSchedule};
use crate::orchestration_files::constants::{
    ATTEMPTS_RANGE, CONCURRENCY_KEY, MAX_POOL_AMOUNT, RETRIES_RANGE, RETRY_ATTEMPTS_KEY,
    RETRY_DELAY_KEY, RETRY_DELAY_RANGE, RETRY_TABLE, SUGGESTION_CUTOFF,
};
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{
    Deprecation, FilePosition, Positioned, RetryPolicy, RunFile, ScheduleFile, ScheduleSource,
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

/// A retry key's value under its current name and under its deprecated name.
type RetryValues = (Option<Spanned<i64>>, Option<Spanned<i64>>);

/// A retry number with whether it was set, and the deprecation its name records.
type RetryNumber<T> = (Option<(T, bool)>, Option<Deprecation>);

/// One retry number within its name's range, and the deprecation a deprecated name records.
fn retry_number<T: TryFrom<i64>>(
    text: &str,
    (table, keys, values): (&str, (&str, &str), RetryValues),
    ranges: ((i64, i64), (i64, i64)),
) -> Result<RetryNumber<T>, FileError> {
    let mut deprecation: Option<Deprecation> = None;
    let (key, value, range) = match values {
        (Some(_), Some(deprecated)) => {
            return Err(error_at(
                text,
                deprecated.span().start,
                format!(
                    "{table} sets both `{}` and its deprecated name `{}`; keep only `{}`",
                    keys.0, keys.1, keys.0
                ),
            ));
        }
        (Some(current), None) => (keys.0, current, ranges.0),
        (None, Some(deprecated)) => {
            deprecation = Some(Deprecation {
                message: format!("`{}` is deprecated; rename it to `{}`", keys.1, keys.0),
                position: position(text, deprecated.span().start),
            });
            (keys.1, deprecated, ranges.1)
        }
        (None, None) => return Ok((None, None)),
    };
    let start: usize = value.span().start;
    let number: i64 = value.into_inner();
    let invalid = || {
        error_at(
            text,
            start,
            format!(
                "{table} {key} must be a whole number from {} to {}",
                range.0, range.1
            ),
        )
    };
    if !(range.0..=range.1).contains(&number) {
        return Err(invalid());
    }
    T::try_from(number)
        .map(|number| (Some((number, key == keys.1)), deprecation))
        .map_err(|_| invalid())
}

/// `[retry]`, or the deprecated `[on_failure]`, checked, with every deprecated name it uses.
fn retry(
    text: &str,
    (current, deprecated): (Option<Spanned<RawRetry>>, Option<Spanned<RawRetry>>),
) -> Result<(Option<RetryPolicy>, Vec<Deprecation>), FileError> {
    let mut deprecations: Vec<Deprecation> = Vec::new();
    let (table, raw) = match (current, deprecated) {
        (Some(_), Some(deprecated)) => {
            return Err(error_at(
                text,
                deprecated.span().start,
                format!(
                    "`[{}]` and its deprecated name `[{}]` are both set; keep only `[{}]`",
                    RETRY_TABLE.0, RETRY_TABLE.1, RETRY_TABLE.0
                ),
            ));
        }
        (Some(current), None) => (RETRY_TABLE.0, current),
        (None, Some(deprecated)) => {
            deprecations.push(Deprecation {
                message: format!(
                    "`[{}]` is deprecated; rename it to `[{}]`",
                    RETRY_TABLE.1, RETRY_TABLE.0
                ),
                position: position(text, deprecated.span().start),
            });
            (RETRY_TABLE.1, deprecated)
        }
        (None, None) => return Ok((None, deprecations)),
    };
    let start: usize = raw.span().start;
    let raw: RawRetry = raw.into_inner();
    let ranges = |range: (u8, u8)| (i64::from(range.0), i64::from(range.1));
    let (attempts, attempts_deprecation) = retry_number::<u8>(
        text,
        (table, RETRY_ATTEMPTS_KEY, (raw.attempts, raw.retries)),
        (ranges(ATTEMPTS_RANGE), ranges(RETRIES_RANGE)),
    )?;
    let attempts: Option<u8> =
        attempts.map(|(number, reruns)| if reruns { number + 1 } else { number });
    let (delay_seconds, delay_deprecation) = retry_number::<u32>(
        text,
        (
            table,
            RETRY_DELAY_KEY,
            (raw.delay_seconds, raw.retry_delay_seconds),
        ),
        {
            let delay = (
                i64::from(RETRY_DELAY_RANGE.0),
                i64::from(RETRY_DELAY_RANGE.1),
            );
            (delay, delay)
        },
    )?;
    let delay_seconds: Option<u32> = delay_seconds.map(|(seconds, _)| seconds);
    deprecations.extend(attempts_deprecation.into_iter().chain(delay_deprecation));
    let attempts: u8 = attempts.ok_or_else(|| {
        error_at(
            text,
            start,
            format!("{table} needs `{}`", RETRY_ATTEMPTS_KEY.0),
        )
    })?;
    Ok((
        Some(RetryPolicy {
            attempts,
            delay_seconds: delay_seconds.unwrap_or(RETRY_DELAY_RANGE.0),
        }),
        deprecations,
    ))
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
    let (retry, deprecations) = retry(text, (raw.retry, raw.on_failure))?;
    Ok(RunFile {
        name,
        description,
        targets: raw.targets,
        owner: owner.map(|owner| owner.value),
        overlap: raw.overlap,
        pools,
        schedule: scheduled,
        retry,
        deprecations,
    })
}

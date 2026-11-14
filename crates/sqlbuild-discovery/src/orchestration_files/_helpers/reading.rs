//! Positions, unknown keys and value checks shared by job and group files.

use std::collections::BTreeSet;

use sqlbuild_core::text::main::close_matches::close_matches;
use toml::Spanned;
use toml::de::DeTable;

use crate::orchestration_files::_helpers::raw::{RawOnFailure, RawRun, RawSchedule};
use crate::orchestration_files::constants::{
    CONCURRENCY_KEY, RETRIES_RANGE, RETRY_DELAY_RANGE, SUGGESTION_CUTOFF,
};
use crate::orchestration_files::models::{
    FileError, FilePosition, OnFailure, Positioned, RunFile, ScheduleFile, ScheduleSource,
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

fn on_failure(text: &str, raw: Spanned<RawOnFailure>) -> Result<OnFailure, FileError> {
    let start: usize = raw.span().start;
    let raw: RawOnFailure = raw.into_inner();
    let invalid = |field: &str, (low, high): (i64, i64)| {
        error_at(
            text,
            start,
            format!("on_failure {field} must be a whole number from {low} to {high}"),
        )
    };
    let retries: u8 = u8::try_from(raw.retries)
        .ok()
        .filter(|value| (RETRIES_RANGE.0..=RETRIES_RANGE.1).contains(value))
        .ok_or_else(|| {
            invalid(
                "retries",
                (i64::from(RETRIES_RANGE.0), i64::from(RETRIES_RANGE.1)),
            )
        })?;
    let retry_delay_seconds: u32 = u32::try_from(raw.retry_delay_seconds)
        .ok()
        .filter(|value| (RETRY_DELAY_RANGE.0..=RETRY_DELAY_RANGE.1).contains(value))
        .ok_or_else(|| {
            invalid(
                "retry_delay_seconds",
                (
                    i64::from(RETRY_DELAY_RANGE.0),
                    i64::from(RETRY_DELAY_RANGE.1),
                ),
            )
        })?;
    Ok(OnFailure {
        retries,
        retry_delay_seconds,
    })
}

/// The fields jobs and groups share, checked.
pub(crate) fn run_file(text: &str, raw: RawRun) -> Result<RunFile, FileError> {
    let owner: Option<String> = raw
        .owner
        .map(|owner| nonblank(text, owner, "owner").map(|owner| owner.value))
        .transpose()?;
    let mut groups: BTreeSet<&str> = BTreeSet::new();
    if let Some(repeated) = raw
        .concurrency_groups
        .iter()
        .find(|group| !groups.insert(group.as_str()))
    {
        return Err(error_at(
            text,
            0,
            format!("concurrency group '{repeated}' is listed twice"),
        ));
    }
    Ok(RunFile {
        name: nonblank(text, raw.name, "name")?,
        description: nonblank(text, raw.description, "description")?.value,
        targets: raw.targets,
        owner,
        overlap: raw.overlap,
        concurrency_groups: raw.concurrency_groups,
        schedule: raw
            .schedule
            .map(|value| schedule(text, value))
            .transpose()?,
        on_failure: raw
            .on_failure
            .map(|value| on_failure(text, value))
            .transpose()?,
    })
}

//! The janitor usage check: who still reads each archive candidate, and whether to keep it.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyString};
use sqlbuild_cli::diff_report::errors::DiffUserError;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_model_config::model_validation::main::fixed_duration_seconds::fixed_duration_seconds;

use crate::bindings::_helpers::boundary::python_calls::{call_with, within};
use crate::bindings::_helpers::build_progress::transient_status::TransientStatusReporter;
use crate::bindings::_helpers::diff_report::support::cli_error;
use crate::bindings::_helpers::janitor::janitor_keys::{Folded, Key, record, tuple};
use crate::bindings::_helpers::janitor::janitor_planning::completed;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;

const START_MESSAGE: &str = "Checking relation usage...";
const INCOMPLETE_USAGE_CODE: &str = "C503";
const INCOMPLETE_USAGE_HELP: &str = "grant the role read access to the warehouse's query history \
     (see `sqb janitor`, \"Usage comes from the warehouse's query history\"), or run without \
     keep_in_use to archive relations whether or not they are read";

/// The usage window and whether in-use candidates are kept.
pub(crate) struct UsageSettings {
    pub(crate) window: String,
    pub(crate) keep_in_use: bool,
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn flag(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

fn lifecycle(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    call_with(
        &host::entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", text(py, "janitor")),
            ("operation_name", text(py, "janitor_usage_check")),
        ],
    )
}

/// The adapter's usage report for the candidates' relations.
fn read_usage<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    candidates: &[Bound<'py, PyAny>],
    window_seconds: i64,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter.py();
    let relations: Vec<Bound<'py, PyAny>> = candidates
        .iter()
        .map(|candidate| native_field(candidate, "relation"))
        .collect::<PyResult<_>>()?;
    within(&lifecycle(py)?, |lifecycle| {
        let report: Bound<'py, PyAny> = call_with(
            &native_field(adapter, "relation_usage")?,
            &[
                ("connection", connection.clone()),
                ("relations", tuple(py, relations)?),
                (
                    "window_seconds",
                    window_seconds.into_pyobject(py)?.into_any(),
                ),
            ],
        )?;
        completed(lifecycle, native_field(&report, "usages")?.len()?)?;
        Ok(report)
    })
}

/// Each reported usage by its relation's case-insensitive identity.
fn usages_by_relation<'py>(
    report: &Bound<'py, PyAny>,
) -> PyResult<HashMap<Folded, Bound<'py, PyAny>>> {
    let mut usages: HashMap<Folded, Bound<'py, PyAny>> = HashMap::new();
    for usage in native_field(report, "usages")?.try_iter()? {
        let usage: Bound<'py, PyAny> = usage?;
        usages.insert(Key::of(&usage)?.folded(), usage);
    }
    Ok(usages)
}

fn completion_message(
    (checked, reason): (bool, Option<String>),
    (in_use, total): (usize, usize),
    elapsed_seconds: f64,
) -> String {
    if !checked {
        return format!(
            "Relation usage not checked: {}. ({elapsed_seconds:.2}s)",
            reason.unwrap_or_default()
        );
    }
    format!("Checked relation usage: {in_use} of {total} in use. ({elapsed_seconds:.2}s)")
}

/// Why `keep_in_use` cannot run: usage was not checked, or the check left something out.
fn incomplete_usage_error(py: Python<'_>, reason: Option<String>, notices: &[String]) -> PyErr {
    let missing: String = match reason {
        Some(reason) if notices.is_empty() => format!("usage was not checked: {reason}"),
        None if notices.is_empty() => "usage was not checked".to_owned(),
        _ => format!("usage was only partly checked: {}", notices.join("; ")),
    };
    cli_error(
        py,
        ErrorId::CliUser,
        &DiffUserError {
            message: format!(
                "keep_in_use needs the usage of every relation to archive, but {missing}; \
                 nothing was archived"
            ),
            code: INCOMPLETE_USAGE_CODE.to_owned(),
            help: Some(INCOMPLETE_USAGE_HELP.to_owned()),
        },
    )
}

/// The plan with each candidate's reads, the usage check, and in-use candidates kept when asked.
pub(crate) fn check_janitor_usage<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (plan, status): (&Bound<'py, PyAny>, &Bound<'py, TransientStatusReporter>),
    settings: &UsageSettings,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    let candidates: Vec<Bound<'py, PyAny>> = native_field(plan, "archive_candidates")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let window_seconds: i64 = fixed_duration_seconds(&settings.window)
        .map_or(i64::MAX, |seconds| {
            i64::try_from(seconds).unwrap_or(i64::MAX)
        });
    let started: Instant = Instant::now();
    TransientStatusReporter::start(status, START_MESSAGE.to_owned())?;
    let report: Bound<'py, PyAny> =
        match read_usage((adapter, connection), &candidates, window_seconds) {
            Ok(report) => report,
            Err(error) => {
                TransientStatusReporter::error(
                    status,
                    &format!(
                        "Relation usage check failed. ({:.2}s)",
                        started.elapsed().as_secs_f64()
                    ),
                )?;
                return Err(error);
            }
        };
    let checked: bool = native_field(&report, "checked")?.is_truthy()?;
    let reason: Bound<'py, PyAny> = native_field(&report, "reason")?;
    let notices: Vec<String> = native_field(&report, "notices")?.extract()?;
    if settings.keep_in_use && (!checked || !notices.is_empty()) {
        TransientStatusReporter::error(
            status,
            &format!(
                "Relation usage check incomplete. ({:.2}s)",
                started.elapsed().as_secs_f64()
            ),
        )?;
        return Err(incomplete_usage_error(py, reason.extract()?, &notices));
    }
    let usages: HashMap<Folded, Bound<'py, PyAny>> = usages_by_relation(&report)?;
    let keep: bool = settings.keep_in_use;
    let mut archived: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut kept_keys: HashSet<Folded> = HashSet::new();
    let mut in_use: usize = 0;
    for candidate in &candidates {
        let key: Folded = Key::of(&native_field(candidate, "relation")?)?.folded();
        let Some(usage) = usages.get(&key) else {
            archived.push(candidate.clone());
            continue;
        };
        in_use += 1;
        let read: Bound<'py, PyAny> = call_with(
            &native_field(candidate, "with_usage")?,
            &[("usage", usage.clone())],
        )?;
        if keep {
            kept_keys.insert(Key::of(&native_field(candidate, "key")?)?.folded());
            kept.push(read);
        } else {
            archived.push(read);
        }
    }
    let mut deletions: Vec<Bound<'py, PyAny>> = Vec::new();
    for deletion in native_field(plan, "archive_deletion_candidates")?.try_iter()? {
        let deletion: Bound<'py, PyAny> = deletion?;
        let original: Bound<'py, PyAny> = native_field(&deletion, "original_key")?;
        if original.is_none() || !kept_keys.contains(&Key::of(&original)?.folded()) {
            deletions.push(deletion);
        }
    }
    TransientStatusReporter::complete(
        status,
        &completion_message(
            (checked, reason.extract()?),
            (in_use, candidates.len()),
            started.elapsed().as_secs_f64(),
        ),
        true,
    )?;
    let check: Bound<'py, PyAny> = record(
        py,
        ExecutorHost::JanitorUsageCheck,
        &[
            ("checked", flag(py, checked)),
            ("window", text(py, &settings.window)),
            ("keep_in_use", flag(py, settings.keep_in_use)),
            ("reason", reason),
            ("notices", native_field(&report, "notices")?),
        ],
    )?;
    call_with(
        &native_field(plan, "with_usage")?,
        &[
            ("archive_candidates", tuple(py, archived)?),
            ("archive_deletion_candidates", tuple(py, deletions)?),
            ("kept_in_use", tuple(py, kept)?),
            ("usage_check", check),
        ],
    )
}

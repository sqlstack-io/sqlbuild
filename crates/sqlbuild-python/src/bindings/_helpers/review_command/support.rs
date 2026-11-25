//! Errors and progress lines shared by the `sqb review` phases.

use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyTypeMethods;
use sqlbuild_analysis::janitor_planning::models::ArchiveTimestamp;
use sqlbuild_analysis::review_command::errors::ReviewPlanError;
use sqlbuild_analysis::review_command::models::ReviewFailure;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::clone::clone_progress::write_line;
use crate::bindings::_helpers::janitor::janitor_keys::{now_utc, timestamp_fields};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::attribute_text::attribute_text as attribute;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::execution_history::main::nullable_text::nullable_text;

/// The code of an unexpected failure inside a review.
const UNEXPECTED_CODE: &str = "C760";
/// The message of a failure that carries none.
const UNEXPLAINED_FAILURE: &str = "review failed";

/// `class(message, code=code, help=help)`.
fn raised(py: Python<'_>, class: ErrorId, failure: &ReviewFailure) -> PyErr {
    new_error(
        py,
        class,
        &failure.message,
        (Some(&failure.code), failure.help.as_deref()),
    )
}

/// `CliUserError(message, code=code, help=help)`.
pub(crate) fn review_error(py: Python<'_>, code: &str, message: &str, help: Option<&str>) -> PyErr {
    raised(
        py,
        ErrorId::CliUser,
        &ReviewFailure {
            code: code.to_owned(),
            message: message.to_owned(),
            help: help.map(str::to_owned),
        },
    )
}

pub(crate) fn plan_error(py: Python<'_>, error: &ReviewPlanError) -> PyErr {
    review_error(py, &error.code, &error.message, error.help.as_deref())
}

/// The code, message and help of `error`, as the report records it.
pub(crate) fn error_parts(py: Python<'_>, error: &PyErr) -> PyResult<ReviewFailure> {
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    let message: String = match attribute(&value, "message")? {
        Some(message) => message,
        None => value.str()?.extract()?,
    };
    let message: String = if message.is_empty() {
        value.get_type().name()?.extract()?
    } else {
        message
    };
    Ok(ReviewFailure {
        code: attribute(&value, "code")?.unwrap_or_else(|| UNEXPECTED_CODE.to_owned()),
        message: if message.is_empty() {
            UNEXPLAINED_FAILURE.to_owned()
        } else {
            message
        },
        help: attribute(&value, "help")?,
    })
}

/// `ReviewFailedError` carrying the reported error, chained to `cause`.
pub(crate) fn failed_error(py: Python<'_>, error: &ReviewFailure, cause: PyErr) -> PyErr {
    let replacement: PyErr = raised(py, ErrorId::ReviewFailed, error);
    replacement.set_cause(py, Some(cause));
    replacement
}

/// One lifecycle phase: a start line, then a completion or failure line with its duration.
pub(crate) fn phase<T>(
    stream: &Bound<'_, PyAny>,
    (start, done, failed): (&str, &str, &str),
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    write_line(stream, &format!("{start}..."))?;
    let started: Instant = Instant::now();
    let outcome: PyResult<T> = body();
    let elapsed: f64 = started.elapsed().as_secs_f64();
    let label: &str = match &outcome {
        Ok(_) => done,
        Err(_) => failed,
    };
    write_line(stream, &format!("{label} ({elapsed:.2}s)"))?;
    outcome
}

/// Where a review's progress goes: stderr when stdout carries `--json`, otherwise stdout.
pub(crate) fn progress_stream(py: Python<'_>, to_stderr: bool) -> PyResult<Bound<'_, PyAny>> {
    imported(py, "sys", if to_stderr { "stderr" } else { "stdout" })
}

/// Run `body` with `sys.stdout` sent to the `progress` stream when stdout carries JSON.
pub(crate) fn quietly<T>(
    (progress, machine): (&Bound<'_, PyAny>, bool),
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    if !machine {
        return body();
    }
    let redirect: Bound<'_, PyAny> = imported(progress.py(), "contextlib", "redirect_stdout")?;
    within(
        &call_with(&redirect, &[("new_target", progress.clone())])?,
        |_| body(),
    )
}

/// The current UTC time and its `YYYYMMDDTHHMMSSZ` form.
pub(crate) fn utc_now_compact(py: Python<'_>) -> PyResult<(ArchiveTimestamp, String)> {
    let fields: ArchiveTimestamp = timestamp_fields(&now_utc(py)?)?;
    let compact: String = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        fields.year, fields.month, fields.day, fields.hour, fields.minute, fields.second
    );
    Ok((fields, compact))
}

/// The current invocation id, when an invocation identity is active.
pub(crate) fn invocation_id(py: Python<'_>) -> PyResult<Option<String>> {
    let identity: Bound<'_, PyAny> = call_with(
        &host_entry(py, ExecutorHost::CurrentExecutionIdentity)?,
        &[],
    )?;
    if identity.is_none() {
        return Ok(None);
    }
    nullable_text(&native_field(&identity, "invocation_id")?)
}

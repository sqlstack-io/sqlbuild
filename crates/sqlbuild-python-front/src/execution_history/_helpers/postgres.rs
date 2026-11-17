//! `PostgresExecutionHistory`: deployed PostgreSQL event-log and run-projection storage.

use crate::execution_history::main::utc::utc;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyDateTime;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::execution_history::constants::POSTGRES_RETRYABLE_SQLSTATES;
use sqlbuild_state::execution_history::models::RunRow;
use sqlbuild_state::execution_history::types::UtcTime;

use crate::execution_history::_helpers::conversion::exact_int;
use crate::execution_history::_helpers::conversion::parsed_status;
use crate::execution_history::_helpers::conversion::storage_error;
use crate::execution_history::_helpers::conversion::utc_time;
use crate::execution_history::_helpers::statements::column;
use sqlbuild_python_base::boundary::main::imported::imported;

pub(crate) const PSYCOPG_MODULE: &str = "psycopg";
pub(crate) const PSYCOPG_ROWS_MODULE: &str = "psycopg.rows";
pub(crate) const CLOSED: &str = "PostgreSQL execution history storage is closed";
pub(crate) const MISSING_DRIVER: &str = "PostgreSQL execution history requires psycopg 3; install sqlbuild[postgres] or 'psycopg[binary]>=3.2'";
pub(crate) const CONNECT_FAILED: &str = "cannot connect to PostgreSQL execution history; verify the secret-resolved DSN, network access, and sqlbuild[postgres] installation";
pub(crate) const READ_FAILED: &str = "PostgreSQL execution history read failed";
pub(crate) const METADATA_INVALID: &str = "PostgreSQL execution history schema metadata is invalid";
pub(crate) const NAMESPACE_MISSING: &str =
    "PostgreSQL execution history cursor namespace is missing";
pub(crate) const STORAGE_ERROR: ExecutorHost = ExecutorHost::ExecutionHistoryStorageError;

/// The `psycopg.Error` class.
fn driver_error_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, PSYCOPG_MODULE, "Error")
}

pub(crate) fn driver_failure(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &driver_error_class(py)?))
}

/// Serialization failures and deadlocks retry while attempts remain.
pub(crate) fn retries_transaction(sqlstate: Option<&str>, attempts: i64, max_retries: i64) -> bool {
    sqlstate.is_some_and(|state| POSTGRES_RETRYABLE_SQLSTATES.contains(&state))
        && attempts < max_retries
}

/// `value` when it is an `int` that is not a `bool`.
pub(crate) fn storage_position(value: &Bound<'_, PyAny>, message: &str) -> PyResult<i64> {
    exact_int(value).ok_or_else(|| storage_error(value.py(), message))
}

/// `value.astimezone(UTC)` when it is a `datetime`.
pub(crate) fn utc_datetime<'py>(
    value: &Bound<'py, PyAny>,
    message: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if !value.is_instance_of::<PyDateTime>() {
        return Err(storage_error(py, message));
    }
    value.call_method1("astimezone", (utc(py)?,))
}

fn optional_utc(value: &Bound<'_, PyAny>) -> PyResult<Option<UtcTime>> {
    if !value.is_instance_of::<PyDateTime>() {
        return Ok(None);
    }
    utc_time(value).map(Some)
}

fn text_column(row: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    column(row, name)?.str()?.extract()
}

/// One `sqlbuild_run_projection` row as a projected run.
pub(crate) fn stored_run(row: &Bound<'_, PyAny>) -> PyResult<RunRow> {
    let py: Python<'_> = row.py();
    let created_at = utc_datetime(
        &column(row, "created_at")?,
        "PostgreSQL run timestamp is invalid",
    )?;
    let last_storage_order: i64 = storage_position(
        &column(row, "last_event_storage_id")?,
        "PostgreSQL run storage position is invalid",
    )?;
    let optional_str = |name: &str| -> PyResult<Option<String>> {
        let value = column(row, name)?;
        if value.is_none() {
            return Ok(None);
        }
        Ok(Some(value.str()?.extract()?))
    };
    Ok(RunRow {
        run_id: text_column(row, "run_id")?,
        invocation_id: text_column(row, "invocation_id")?,
        created_at: utc_time(&created_at)?,
        status: parsed_status(py, &text_column(row, "status")?)?,
        is_complete: column(row, "is_complete")?.is_truthy()?,
        last_event_cursor: text_column(row, "last_event_cursor")?,
        last_storage_order,
        command: optional_str("command")?,
        target: optional_str("target")?,
        environment: optional_str("environment")?,
        started_at: optional_utc(&column(row, "started_at")?)?,
        ended_at: optional_utc(&column(row, "ended_at")?)?,
    })
}

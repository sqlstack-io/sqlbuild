//! `SQLiteExecutionHistory`: project-local SQLite event-log and run-projection storage.

use crate::execution_history::main::nullable_text::nullable_text;
use std::fs::DirBuilder;
use std::io::ErrorKind;
use std::path::Path;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::execution_history::constants::MEMORY_PATH;
use sqlbuild_state::execution_history::models::RunRow;
use sqlbuild_state::execution_history::types::UtcTime;

use crate::execution_history::_helpers::conversion::parsed_status;
use crate::execution_history::_helpers::conversion::storage_error;
use crate::execution_history::_helpers::conversion::storage_error_from;
use crate::execution_history::_helpers::conversion::utc_time;
use crate::execution_history::_helpers::statements::column;
use sqlbuild_python_base::boundary::main::imported::imported;

const SQLITE_MODULE: &str = "sqlite3";
pub(crate) const PATHLIB_MODULE: &str = "pathlib";
pub(crate) const CLOSED: &str = "SQLite execution history storage is closed";
const NOT_AWARE: &str = "SQLite history timestamp is not timezone-aware";
pub(crate) const INTEGRITY_CONFLICT: ExecutorHost = ExecutorHost::IntegrityConflictError;
pub(crate) const STORAGE_ERROR: ExecutorHost = ExecutorHost::ExecutionHistoryStorageError;

/// `Path(path).parent.mkdir(mode=0o700, parents=True, exist_ok=True)` for a file-backed history.
pub(crate) fn prepare_history_directory(path: &str) -> std::io::Result<()> {
    if path == MEMORY_PATH {
        return Ok(());
    }
    let Some(parent) = Path::new(path).parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() || parent.is_dir() {
        return Ok(());
    }
    if let Some(grandparent) = parent.parent()
        && !grandparent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(grandparent)?;
    }
    match private_directory_builder().create(parent) {
        Err(error) if error.kind() == ErrorKind::AlreadyExists && parent.is_dir() => Ok(()),
        result => result,
    }
}

/// A directory builder with the owner-only mode Python's `os.mkdir(mode=0o700)` applies.
#[cfg(unix)]
fn private_directory_builder() -> DirBuilder {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder: DirBuilder = DirBuilder::new();
    builder.mode(sqlbuild_state::execution_history::constants::SQLITE_DIRECTORY_MODE);
    builder
}

/// Windows ignores the directory mode, as `os.mkdir` does there.
#[cfg(not(unix))]
fn private_directory_builder() -> DirBuilder {
    DirBuilder::new()
}

/// `os.chmod(path, 0o600)` for a file-backed history.
pub(crate) fn restrict_history_file(path: &str) -> std::io::Result<()> {
    if path == MEMORY_PATH {
        return Ok(());
    }
    set_owner_only(path)
}

#[cfg(unix)]
fn set_owner_only(path: &str) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(
            sqlbuild_state::execution_history::constants::SQLITE_FILE_MODE,
        ),
    )
}

/// `os.chmod(path, 0o600)` on Windows only keeps the file writable, which it already is.
#[cfg(not(unix))]
fn set_owner_only(_path: &str) -> std::io::Result<()> {
    Ok(())
}

/// Whether `error` is a `sqlite3.Error`.
pub(crate) fn sqlite_failure(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &imported(py, SQLITE_MODULE, "Error")?))
}

/// `result`, with a `sqlite3.Error` raised as a storage error `message` from it.
pub(crate) fn wrapped<T>(py: Python<'_>, result: PyResult<T>, message: &str) -> PyResult<T> {
    match result {
        Err(error) if sqlite_failure(py, &error)? => Err(storage_error_from(py, message, error)),
        result => result,
    }
}

/// A stored ISO timestamp as an aware UTC `datetime`.
pub(crate) fn parsed_timestamp<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let text: String = value.extract()?;
    let parsed: Bound<'py, PyAny> = imported(py, "datetime", "datetime")?
        .call_method1("fromisoformat", (text.replace('Z', "+00:00"),))?;
    if parsed.call_method0("utcoffset")?.is_none() {
        return Err(storage_error(py, NOT_AWARE));
    }
    parsed.call_method1("astimezone", (imported(py, "datetime", "UTC")?,))
}

fn optional_timestamp(value: &Bound<'_, PyAny>) -> PyResult<Option<UtcTime>> {
    if value.is_none() {
        return Ok(None);
    }
    utc_time(&parsed_timestamp(value)?).map(Some)
}

/// One `run_projection` row as a projected run.
pub(crate) fn stored_run(row: &Bound<'_, PyAny>) -> PyResult<RunRow> {
    let py: Python<'_> = row.py();
    let status: String = column(row, "status")?.extract()?;
    Ok(RunRow {
        run_id: column(row, "run_id")?.extract()?,
        invocation_id: column(row, "invocation_id")?.extract()?,
        created_at: utc_time(&parsed_timestamp(&column(row, "created_at")?)?)?,
        status: parsed_status(py, &status)?,
        is_complete: column(row, "is_complete")?.is_truthy()?,
        last_event_cursor: column(row, "last_event_cursor")?.extract()?,
        last_storage_order: column(row, "last_storage_order")?.extract()?,
        command: nullable_text(&column(row, "command")?)?,
        target: nullable_text(&column(row, "target")?)?,
        environment: nullable_text(&column(row, "environment")?)?,
        started_at: optional_timestamp(&column(row, "started_at")?)?,
        ended_at: optional_timestamp(&column(row, "ended_at")?)?,
    })
}

//! `SQLiteExecutionHistory`: project-local SQLite event-log and run-projection storage.

use std::fs::DirBuilder;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyclass, pymethods};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::execution_history::constants::{
    DEFAULT_PAGE_LIMIT, MEMORY_PATH, RUN_CURSOR_TIMESTAMP, SCHEMA_VERSION, SQLITE_ALL_EVENTS,
    SQLITE_ALL_RUNS, SQLITE_BEGIN, SQLITE_BUSY_TIMEOUT_MS, SQLITE_COMMIT, SQLITE_DELETE_RUNS,
    SQLITE_EVENT_BY_ID, SQLITE_EVENT_BY_STORAGE_ID, SQLITE_EVENT_EXISTS, SQLITE_FOREIGN_KEYS,
    SQLITE_HEALTHY_CHECK_RESULT, SQLITE_INSERT_NAMESPACE, SQLITE_INSERT_SCHEMA_VERSION,
    SQLITE_JOURNAL_MODE, SQLITE_METADATA_EXISTS, SQLITE_METADATA_TABLE, SQLITE_METADATA_VALUE,
    SQLITE_NAMESPACE_KEY, SQLITE_QUICK_CHECK, SQLITE_ROLLBACK, SQLITE_RUN_BY_ID, SQLITE_RUN_EXISTS,
    SQLITE_RUN_INSERT, SQLITE_SCHEMA, SQLITE_SCHEMA_VERSION_KEY, UNKNOWN_EVENT_CURSOR,
    UNKNOWN_RUN_CURSOR,
};
use sqlbuild_state::execution_history::errors::HistoryErrorKind;
use sqlbuild_state::execution_history::main::decoded_run_cursor::decoded_run_cursor;
use sqlbuild_state::execution_history::main::event_cursor::event_cursor;
use sqlbuild_state::execution_history::main::event_cursor_position::event_cursor_position;
use sqlbuild_state::execution_history::main::event_insert_query::event_insert_query;
use sqlbuild_state::execution_history::main::event_page_query::event_page_query;
use sqlbuild_state::execution_history::main::project_runs::project_runs;
use sqlbuild_state::execution_history::main::run_cursor::run_cursor;
use sqlbuild_state::execution_history::main::run_insert_query::run_insert_query;
use sqlbuild_state::execution_history::main::run_page_query::run_page_query;
use sqlbuild_state::execution_history::main::schema_migration_needed::schema_migration_needed;
use sqlbuild_state::execution_history::main::sqlite_database_path::sqlite_database_path;
use sqlbuild_state::execution_history::main::validate_page_limit::validate_page_limit;
use sqlbuild_state::execution_history::main::validate_schema_target::validate_schema_target;
use sqlbuild_state::execution_history::main::validate_stored_content::validate_stored_content;
use sqlbuild_state::execution_history::models::{
    Dialect, PreparedEvent, RunCursorKey, RunRow, SqlParam, SqliteLocation,
};
use sqlbuild_state::execution_history::types::UtcTime;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::execution_history::appends::prepared_batch;
use crate::bindings::_helpers::execution_history::conversion::{
    checked, event_filter_spec, exact_int, history_error_of_kind, new_namespace, now,
    optional_text, parsed_iso, parsed_status, projection_facts, python_params, rejection,
    run_filter_spec, run_records, run_rows, storage_error, storage_error_from, stored_event,
    text_of, utc_time,
};
use crate::bindings::_helpers::execution_history::statements::{
    column, execute, execute_plain, execute_query, fetch_one, tuple_of,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::sqlite_connection::{SqliteConnection, sqlite_params};
use crate::bindings::_helpers::native_host::sqlite_cursor::SqliteCursor;
use crate::bindings::_helpers::native_threads::reentrant_lock::ReentrantLock;
use crate::bindings::_helpers::records::fields::field as native_field;

const SQLITE_MODULE: &str = "sqlite3";
const PATHLIB_MODULE: &str = "pathlib";
const CLOSED: &str = "SQLite execution history storage is closed";
const NOT_AWARE: &str = "SQLite history timestamp is not timezone-aware";
const INTEGRITY_CONFLICT: ExecutorHost = ExecutorHost::IntegrityConflictError;
const STORAGE_ERROR: ExecutorHost = ExecutorHost::ExecutionHistoryStorageError;

/// `Path(path).parent.mkdir(mode=0o700, parents=True, exist_ok=True)` for a file-backed history.
fn prepare_history_directory(path: &str) -> std::io::Result<()> {
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
fn restrict_history_file(path: &str) -> std::io::Result<()> {
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
fn sqlite_failure(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &imported(py, SQLITE_MODULE, "Error")?))
}

/// `result`, with a `sqlite3.Error` raised as a storage error `message` from it.
fn wrapped<T>(py: Python<'_>, result: PyResult<T>, message: &str) -> PyResult<T> {
    match result {
        Err(error) if sqlite_failure(py, &error)? => Err(storage_error_from(py, message, error)),
        result => result,
    }
}

/// A stored ISO timestamp as an aware UTC `datetime`.
fn parsed_timestamp<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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
fn stored_run(row: &Bound<'_, PyAny>) -> PyResult<RunRow> {
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
        command: optional_text(&column(row, "command")?)?,
        target: optional_text(&column(row, "target")?)?,
        environment: optional_text(&column(row, "environment")?)?,
        started_at: optional_timestamp(&column(row, "started_at")?)?,
        ended_at: optional_timestamp(&column(row, "ended_at")?)?,
    })
}

/// Project-local SQLite implementation of event-log and run-projection storage contracts.
#[pyclass(
    module = "sqlbuild._native",
    name = "SQLiteExecutionHistory",
    frozen,
    subclass,
    dict
)]
pub(crate) struct SqliteExecutionHistory {
    path: OnceLock<String>,
    lock: ReentrantLock,
    closed: AtomicBool,
    project_calls: AtomicI64,
}

impl SqliteExecutionHistory {
    fn database_path(&self) -> &str {
        self.path.get().map_or(MEMORY_PATH, String::as_str)
    }

    fn with_lock<'py, T>(
        &self,
        py: Python<'py>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        self.lock.with_lock(py, body)
    }

    fn ensure_open(&self, py: Python<'_>) -> PyResult<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(storage_error(py, CLOSED));
        }
        Ok(())
    }

    /// Run `body` in `BEGIN IMMEDIATE`, committing on success and rolling back on failure.
    fn transaction<'py, T>(
        &self,
        slf: &Bound<'py, Self>,
        body: impl FnOnce(&Bound<'py, PyAny>) -> PyResult<T>,
    ) -> PyResult<T> {
        let py: Python<'py> = slf.py();
        self.with_lock(py, || {
            self.ensure_open(py)?;
            let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
            execute_plain(&connection, SQLITE_BEGIN)?;
            let value: T = match body(&connection) {
                Ok(value) => value,
                Err(error) => {
                    let _ = execute_plain(&connection, SQLITE_ROLLBACK);
                    return Err(error);
                }
            };
            if let Err(error) = execute_plain(&connection, SQLITE_COMMIT) {
                if sqlite_failure(py, &error)?
                    && match connection.cast::<SqliteConnection>() {
                        Ok(connection) => connection.get().in_transaction(py)?,
                        Err(_) => native_field(&connection, "in_transaction")?.is_truthy()?,
                    }
                {
                    let _ = execute_plain(&connection, SQLITE_ROLLBACK);
                }
                return Err(error);
            }
            Ok(value)
        })
    }

    fn metadata_value<'py>(
        connection: &Bound<'py, PyAny>,
        key: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let row = fetch_one(&execute(
            connection,
            SQLITE_METADATA_VALUE,
            &[SqlParam::Text(key.to_owned())],
        )?)?;
        row.map(|row| row.get_item(0)).transpose()
    }

    fn metadata_version(connection: &Bound<'_, PyAny>) -> PyResult<i64> {
        let py: Python<'_> = connection.py();
        let exists = fetch_one(&execute(
            connection,
            SQLITE_METADATA_EXISTS,
            &[SqlParam::Text(SQLITE_METADATA_TABLE.to_owned())],
        )?)?;
        if exists.is_none() {
            return Ok(0);
        }
        let Some(value) = Self::metadata_value(connection, SQLITE_SCHEMA_VERSION_KEY)? else {
            return Ok(0);
        };
        let invalid = || storage_error(py, "SQLite history schema metadata is invalid");
        match text_of(&value)? {
            Some(text) => text.trim().parse::<i64>().map_err(|_| invalid()),
            None => exact_int(&value).ok_or_else(invalid),
        }
    }

    fn namespace(connection: &Bound<'_, PyAny>) -> PyResult<String> {
        let py: Python<'_> = connection.py();
        match Self::metadata_value(connection, SQLITE_NAMESPACE_KEY)? {
            Some(value) => Ok(value.str()?.extract()?),
            None => Err(storage_error(
                py,
                "SQLite history cursor namespace is missing",
            )),
        }
    }

    fn stored_event<'py>(row: &Bound<'py, PyAny>, namespace: &str) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = row.py();
        let storage_id: i64 = column(row, "storage_id")?.extract()?;
        stored_event(
            py,
            (
                storage_id,
                event_cursor(Dialect::Sqlite, namespace, storage_id),
            ),
            parsed_timestamp(&column(row, "received_at")?)?,
            &column(row, "payload_json")?,
        )
    }

    fn read_all_runs(connection: &Bound<'_, PyAny>) -> PyResult<Vec<RunRow>> {
        execute_plain(connection, SQLITE_ALL_RUNS)?
            .try_iter()?
            .map(|row| row.and_then(|row| stored_run(&row)))
            .collect()
    }

    fn replace_projection(connection: &Bound<'_, PyAny>, projected: &[RunRow]) -> PyResult<()> {
        let py: Python<'_> = connection.py();
        execute_plain(connection, SQLITE_DELETE_RUNS)?;
        if let Ok(connection) = connection.cast::<SqliteConnection>() {
            for run in projected {
                connection.get().execute_native(
                    py,
                    SQLITE_RUN_INSERT,
                    &sqlite_params(&run_insert_query(Dialect::Sqlite, run).params)?,
                )?;
            }
            return Ok(());
        }
        let rows: Vec<Bound<'_, PyAny>> = projected
            .iter()
            .map(|run| {
                python_params(py, &run_insert_query(Dialect::Sqlite, run).params)
                    .map(Bound::into_any)
            })
            .collect::<PyResult<_>>()?;
        connection.call_method1("executemany", (SQLITE_RUN_INSERT, tuple_of(py, rows)?))?;
        Ok(())
    }

    fn append_prepared<'py>(
        connection: &Bound<'py, PyAny>,
        prepared: &[PreparedEvent],
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py: Python<'py> = connection.py();
        let mut stored: Vec<Bound<'py, PyAny>> = Vec::with_capacity(prepared.len());
        let mut namespace: Option<String> = None;
        for event in prepared {
            let existing = fetch_one(&execute(
                connection,
                SQLITE_EVENT_BY_ID,
                &[SqlParam::Text(event.event_id.clone())],
            )?)?;
            let row: Bound<'py, PyAny> = match existing {
                Some(existing) => {
                    let digest: Option<String> = text_of(&column(&existing, "content_digest")?)?;
                    let payload: Option<String> = text_of(&column(&existing, "payload_json")?)?;
                    checked(
                        py,
                        validate_stored_content(event, (digest.as_deref(), payload.as_deref())),
                    )?;
                    existing
                }
                None => {
                    let query = event_insert_query(Dialect::Sqlite, event, &now(py)?);
                    let cursor: Bound<'py, PyAny> = execute_query(connection, &query)?;
                    let storage_id: i64 = match cursor.cast::<SqliteCursor>() {
                        Ok(cursor) => cursor.get().last_row_id,
                        Err(_) => native_field(&cursor, "lastrowid")?.extract()?,
                    };
                    fetch_one(&execute(
                        connection,
                        SQLITE_EVENT_BY_STORAGE_ID,
                        &[SqlParam::Integer(storage_id)],
                    )?)?
                    .unwrap_or_else(|| py.None().into_bound(py))
                }
            };
            if namespace.is_none() {
                namespace = Some(Self::namespace(connection)?);
            }
            let namespace: &str = namespace.as_deref().unwrap_or_default();
            stored.push(Self::stored_event(&row, namespace)?);
        }
        Ok(stored)
    }

    fn append_events_in<'py>(
        connection: &Bound<'py, PyAny>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let prepared: Vec<PreparedEvent> = prepared_batch(events)?;
        Self::append_prepared(connection, &prepared)
    }

    fn migrate_in(&self, slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let connection: Bound<'_, PyAny> = native_field(slf, "_connection")?;
        let current: i64 = Self::metadata_version(&connection)?;
        if !checked(py, schema_migration_needed(Dialect::Sqlite, current))? {
            return Ok(());
        }
        let namespace: String = new_namespace(py)?;
        let migrated = self.transaction(slf, |connection| {
            for statement in SQLITE_SCHEMA {
                execute_plain(connection, statement)?;
            }
            execute(
                connection,
                SQLITE_INSERT_NAMESPACE,
                &[
                    SqlParam::Text(SQLITE_NAMESPACE_KEY.to_owned()),
                    SqlParam::Text(namespace.clone()),
                ],
            )?;
            execute(
                connection,
                SQLITE_INSERT_SCHEMA_VERSION,
                &[
                    SqlParam::Text(SQLITE_SCHEMA_VERSION_KEY.to_owned()),
                    SqlParam::Text(SCHEMA_VERSION.to_string()),
                ],
            )?;
            Ok(())
        });
        wrapped(py, migrated, "SQLite history schema migration failed")
    }

    fn run_cursor_key(
        connection: &Bound<'_, PyAny>,
        cursor: &Bound<'_, PyAny>,
    ) -> PyResult<Option<RunCursorKey>> {
        let py: Python<'_> = connection.py();
        if cursor.is_none() {
            return Ok(None);
        }
        let text: String = text_of(cursor)?.unwrap_or_default();
        let namespace: String = Self::namespace(connection)?;
        let decoded = checked(py, decoded_run_cursor(Dialect::Sqlite, &text, &namespace))?;
        let invalid_timestamp =
            || rejection(py, HistoryErrorKind::InvalidCursor, RUN_CURSOR_TIMESTAMP);
        let created_at = match parsed_iso(py, &decoded.created_at_text)? {
            Some(parsed) if !parsed.call_method0("utcoffset")?.is_none() => utc_time(&parsed)?,
            _ => return Err(invalid_timestamp()),
        };
        let exists = fetch_one(&execute(
            connection,
            SQLITE_RUN_EXISTS,
            &[
                SqlParam::Text(decoded.created_at_text.clone()),
                SqlParam::Text(decoded.run_id.clone()),
            ],
        )?)?;
        if exists.is_none() {
            return Err(rejection(
                py,
                HistoryErrorKind::InvalidCursor,
                UNKNOWN_RUN_CURSOR,
            ));
        }
        Ok(Some(RunCursorKey {
            created_at,
            created_at_text: decoded.created_at_text,
            run_id: decoded.run_id,
        }))
    }
}

#[pymethods]
impl SqliteExecutionHistory {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        Self {
            path: OnceLock::new(),
            lock: ReentrantLock::default(),
            closed: AtomicBool::new(true),
            project_calls: AtomicI64::new(0),
        }
    }

    #[pyo3(signature = (project_dir = None, *, path = None, busy_timeout_ms = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        project_dir: Option<&Bound<'_, PyAny>>,
        path: Option<&Bound<'_, PyAny>>,
        busy_timeout_ms: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        let fspath = |value: Option<&Bound<'_, PyAny>>| -> PyResult<Option<String>> {
            value
                .filter(|value| !value.is_none())
                .map(|value| imported(py, "os", "fspath")?.call1((value,))?.extract())
                .transpose()
        };
        let project_dir: Option<String> = fspath(project_dir)?;
        let path: Option<String> = fspath(path)?;
        let working_directory: String = if project_dir.is_none() && path.is_none() {
            std::env::current_dir()?.to_string_lossy().into_owned()
        } else {
            String::new()
        };
        let location = SqliteLocation {
            project_dir,
            path,
            working_directory,
            busy_timeout_ms: busy_timeout_ms.map_or(Some(SQLITE_BUSY_TIMEOUT_MS), exact_int),
        };
        let database: String = checked(py, sqlite_database_path(&location))?;
        let busy_timeout_ms: i64 = location.busy_timeout_ms.unwrap_or(SQLITE_BUSY_TIMEOUT_MS);
        let _ = this.path.set(database.clone());
        this.closed.store(false, Ordering::SeqCst);
        let mut connection: Option<Bound<'_, PyAny>> = None;
        let opened: PyResult<()> = (|| {
            prepare_history_directory(&database)?;
            let opened = Bound::new(py, SqliteConnection::open(py, &database)?)?.into_any();
            connection = Some(opened.clone());
            slf.setattr("_connection", &opened)?;
            this.with_lock(py, || {
                execute_plain(&opened, &format!("PRAGMA busy_timeout = {busy_timeout_ms}"))?;
                execute_plain(&opened, SQLITE_FOREIGN_KEYS)?;
                if database != MEMORY_PATH {
                    execute_plain(&opened, SQLITE_JOURNAL_MODE)?.call_method0("fetchone")?;
                }
                slf.call_method0("_migrate")?;
                Ok(())
            })?;
            restrict_history_file(&database)?;
            slf.call_method0("reconcile")?;
            Ok(())
        })();
        let Err(error) = opened else {
            return Ok(());
        };
        let is_storage: bool = history_error_of_kind(py, &error, STORAGE_ERROR)?;
        let is_open_failure: bool =
            error.is_instance_of::<pyo3::exceptions::PyOSError>(py) || sqlite_failure(py, &error)?;
        if !is_storage && !is_open_failure {
            return Err(error);
        }
        if let Some(connection) = connection {
            let _ = connection.call_method0("close");
        }
        this.closed.store(true, Ordering::SeqCst);
        if is_storage {
            return Err(error);
        }
        Err(storage_error_from(
            py,
            &format!("cannot open SQLite history at {database}"),
            error,
        ))
    }

    /// The database path, or `None` for an in-memory database.
    #[getter]
    pub(crate) fn path<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let path: &str = self.database_path();
        if path == MEMORY_PATH {
            return Ok(py.None().into_bound(py));
        }
        imported(py, PATHLIB_MODULE, "Path")?.call1((path,))
    }

    /// The number of incremental projection calls.
    #[getter]
    pub(crate) fn project_calls(&self) -> i64 {
        self.project_calls.load(Ordering::SeqCst)
    }

    fn append_event<'py>(
        slf: &Bound<'py, Self>,
        event: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            slf.call_method1("append_events", (tuple_of(py, vec![event.clone()])?,))?
                .get_item(0)
        })
    }

    fn append_events<'py>(
        slf: &Bound<'py, Self>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let pending: Bound<'py, PyTuple> =
                PyTuple::new(py, events.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
            let appended = this.transaction(slf, |connection| {
                Self::append_events_in(connection, pending.as_any())
            });
            let stored = Self::without_conflicts(py, appended, "SQLite event append failed")?;
            tuple_of(py, stored)
        })
    }

    #[pyo3(signature = (*, event_filter, after_cursor = None, limit = None))]
    #[pyo3(name = "get_events")]
    fn stored_events_page<'py>(
        slf: &Bound<'py, Self>,
        event_filter: &Bound<'py, PyAny>,
        after_cursor: Option<&Bound<'py, PyAny>>,
        limit: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let limit: Option<i64> = limit.map_or(Some(DEFAULT_PAGE_LIMIT), exact_int);
            checked(py, validate_page_limit(limit))?;
            let limit: i64 = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
            let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
            let after: i64 = Self::event_position(&connection, after_cursor)?;
            let spec = event_filter_spec(event_filter)?;
            let query = event_page_query(Dialect::Sqlite, after, &spec, limit);
            let rows: Vec<Bound<'py, PyAny>> = wrapped(
                py,
                execute_query(&connection, &query)
                    .and_then(|cursor| cursor.try_iter()?.collect::<PyResult<Vec<_>>>()),
                "SQLite event read failed",
            )?;
            let has_more: bool = rows.len() as i64 > limit;
            let mut records: Vec<Bound<'py, PyAny>> = Vec::new();
            if !rows.is_empty() {
                let namespace: String = Self::namespace(&connection)?;
                for row in rows.iter().take(limit as usize) {
                    records.push(Self::stored_event(row, &namespace)?);
                }
            }
            let next_cursor: Bound<'py, PyAny> = match records.last() {
                Some(record) => native_field(record, "cursor")?,
                None => py.None().into_bound(py),
            };
            crate::bindings::_helpers::boundary::python_calls::call_with(
                &host::entry(py, ExecutorHost::EventPage)?,
                &[
                    ("records", tuple_of(py, records)?.into_any()),
                    ("next_cursor", next_cursor),
                    ("has_more", has_more.into_bound_py_any(py)?),
                ],
            )
        })
    }

    #[pyo3(name = "get_run")]
    fn run_record<'py>(
        slf: &Bound<'py, Self>,
        run_id: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
            let row =
                fetch_one(&connection.call_method1("execute", (SQLITE_RUN_BY_ID, (run_id,)))?)?;
            match row {
                Some(row) => crate::bindings::_helpers::execution_history::conversion::run_record(
                    py,
                    &stored_run(&row)?,
                ),
                None => Ok(py.None().into_bound(py)),
            }
        })
    }

    #[pyo3(signature = (*, run_filter, after_cursor = None, limit = None))]
    #[pyo3(name = "get_runs")]
    fn run_page<'py>(
        slf: &Bound<'py, Self>,
        run_filter: &Bound<'py, PyAny>,
        after_cursor: Option<&Bound<'py, PyAny>>,
        limit: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let limit: Option<i64> = limit.map_or(Some(DEFAULT_PAGE_LIMIT), exact_int);
            checked(py, validate_page_limit(limit))?;
            let limit: i64 = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
            let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
            let none = py.None().into_bound(py);
            let after = Self::run_cursor_key(&connection, after_cursor.unwrap_or(&none))?;
            let spec = run_filter_spec(run_filter)?;
            let query = run_page_query(Dialect::Sqlite, after.as_ref(), &spec, limit);
            let rows: Vec<Bound<'py, PyAny>> = execute_query(&connection, &query)?
                .try_iter()?
                .collect::<PyResult<_>>()?;
            let runs: Vec<RunRow> = rows
                .iter()
                .take(limit as usize)
                .map(stored_run)
                .collect::<PyResult<_>>()?;
            let next_cursor: Bound<'py, PyAny> = match runs.last() {
                Some(run) => {
                    let namespace: String = Self::namespace(&connection)?;
                    checked(py, run_cursor(Dialect::Sqlite, &namespace, run))?
                        .into_bound_py_any(py)?
                }
                None => py.None().into_bound(py),
            };
            crate::bindings::_helpers::boundary::python_calls::call_with(
                &host::entry(py, ExecutorHost::RunPage)?,
                &[
                    ("records", run_records(py, &runs)?.into_any()),
                    ("next_cursor", next_cursor),
                    (
                        "has_more",
                        (rows.len() as i64 > limit).into_bound_py_any(py)?,
                    ),
                ],
            )
        })
    }

    fn project<'py>(
        slf: &Bound<'py, Self>,
        stored_events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            this.project_calls.fetch_add(1, Ordering::SeqCst);
            let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
            let current: Vec<RunRow> = Self::read_all_runs(&connection)?;
            let facts = projection_facts(stored_events)?;
            let projected: Vec<RunRow> = checked(py, project_runs(facts, current))?;
            let records: Bound<'py, PyTuple> = run_records(py, &projected)?;
            slf.call_method1("_publish_projection", (&records,))?;
            Ok(records)
        })
    }

    fn rebuild_from_events<'py>(
        slf: &Bound<'py, Self>,
        stored_events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let facts = projection_facts(stored_events)?;
            let projected: Vec<RunRow> = checked(py, project_runs(facts, Vec::new()))?;
            let records: Bound<'py, PyTuple> = run_records(py, &projected)?;
            slf.call_method1("_publish_projection", (&records,))?;
            Ok(records)
        })
    }

    /// Atomically append immutable facts and publish their run projection.
    fn append_and_project<'py>(
        slf: &Bound<'py, Self>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let pending: Bound<'py, PyTuple> =
                PyTuple::new(py, events.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
            let appended = this.transaction(slf, |connection| {
                let stored: Vec<Bound<'py, PyAny>> =
                    Self::append_events_in(connection, pending.as_any())?;
                let stored_tuple: Bound<'py, PyTuple> = tuple_of(py, stored.clone())?;
                let facts = projection_facts(stored_tuple.as_any())?;
                let projected = checked(py, project_runs(facts, Self::read_all_runs(connection)?))?;
                Self::replace_projection(connection, &projected)?;
                Ok(stored)
            });
            let stored = Self::without_conflicts(
                py,
                appended,
                "SQLite atomic append and projection failed",
            )?;
            tuple_of(py, stored)
        })
    }

    /// Rebuild the disposable run projection from all durable event facts.
    fn reconcile<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let reconciled = this.transaction(slf, |connection| {
                let records: Bound<'py, PyAny> = slf.call_method0("_read_all_events")?;
                let facts = projection_facts(&records)?;
                let projected: Vec<RunRow> = checked(py, project_runs(facts, Vec::new()))?;
                Self::replace_projection(connection, &projected)?;
                run_records(py, &projected)
            });
            wrapped(py, reconciled, "SQLite reconciliation failed")
        })
    }

    /// Verify connectivity, integrity, and the supported schema revision.
    fn check_health(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let connection: Bound<'_, PyAny> = native_field(slf, "_connection")?;
            let Some(result) = fetch_one(&execute_plain(&connection, SQLITE_QUICK_CHECK)?)? else {
                return Ok(false);
            };
            if text_of(&result.get_item(0)?)?.as_deref() != Some(SQLITE_HEALTHY_CHECK_RESULT) {
                return Ok(false);
            }
            let version: i64 = slf.call_method0("get_schema_version")?.extract()?;
            Ok(version == SCHEMA_VERSION)
        })
    }

    #[pyo3(name = "get_schema_version")]
    fn schema_version(slf: &Bound<'_, Self>) -> PyResult<i64> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            Self::metadata_version(&native_field(slf, "_connection")?)
        })
    }

    #[pyo3(signature = (*, target_version = None))]
    fn upgrade_schema(slf: &Bound<'_, Self>, target_version: Option<i64>) -> PyResult<i64> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let target: i64 = target_version.unwrap_or(SCHEMA_VERSION);
            checked(py, validate_schema_target(Dialect::Sqlite, target))?;
            slf.call_method0("_migrate")?;
            Ok(target)
        })
    }

    fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            if this.closed.load(Ordering::SeqCst) {
                return Ok(());
            }
            let closed = native_field(slf, "_connection")
                .and_then(|connection| connection.call_method0("close"));
            this.closed.store(true, Ordering::SeqCst);
            closed.map(|_| ())
        })
    }

    fn dispose(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.call_method0("close")?;
        Ok(())
    }

    fn __enter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || this.ensure_open(py))?;
        Ok(slf.clone())
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(slf: &Bound<'_, Self>, _exc_info: &Bound<'_, PyTuple>) -> PyResult<Option<bool>> {
        slf.call_method0("close")?;
        Ok(None)
    }

    fn _migrate(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.get().migrate_in(slf)
    }

    fn _read_all_events<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
        let rows: Vec<Bound<'py, PyAny>> = execute_plain(&connection, SQLITE_ALL_EVENTS)?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(rows.len());
        if !rows.is_empty() {
            let namespace: String = Self::namespace(&connection)?;
            for row in &rows {
                records.push(Self::stored_event(row, &namespace)?);
            }
        }
        tuple_of(py, records)
    }

    fn _publish_projection(slf: &Bound<'_, Self>, projected: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let runs: Vec<RunRow> = run_rows(projected)?;
        let published = slf.get().transaction(slf, |connection| {
            Self::replace_projection(connection, &runs)
        });
        wrapped(py, published, "SQLite run projection publication failed")
    }
}

impl SqliteExecutionHistory {
    /// `result` with integrity conflicts kept and `sqlite3.Error` wrapped as `message`.
    fn without_conflicts<T>(py: Python<'_>, result: PyResult<T>, message: &str) -> PyResult<T> {
        match result {
            Err(error) if history_error_of_kind(py, &error, INTEGRITY_CONFLICT)? => Err(error),
            result => wrapped(py, result, message),
        }
    }

    fn event_position(
        connection: &Bound<'_, PyAny>,
        cursor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<i64> {
        let py: Python<'_> = connection.py();
        let Some(cursor) = cursor.filter(|cursor| !cursor.is_none()) else {
            return Ok(0);
        };
        let text: String = text_of(cursor)?.unwrap_or_default();
        let namespace: String = Self::namespace(connection)?;
        let position: i64 = checked(
            py,
            event_cursor_position(Dialect::Sqlite, Some(&text), &namespace),
        )?;
        let exists = fetch_one(&execute(
            connection,
            SQLITE_EVENT_EXISTS,
            &[SqlParam::Integer(position)],
        )?)?;
        if exists.is_none() {
            return Err(rejection(
                py,
                HistoryErrorKind::InvalidCursor,
                UNKNOWN_EVENT_CURSOR,
            ));
        }
        Ok(position)
    }
}

//! `PostgresExecutionHistory`: deployed PostgreSQL event-log and run-projection storage.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyclass, pymethods};
use pyo3::types::{PyBool, PyDateTime, PyDict, PyTuple};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::execution_history::constants::{
    DEFAULT_PAGE_LIMIT, POSTGRES_ADVISORY_LOCK, POSTGRES_ALL_EVENTS, POSTGRES_ALL_RUNS,
    POSTGRES_CONNECT_TIMEOUT_SECONDS, POSTGRES_DELETE_RUNS, POSTGRES_EVENT_BY_ID,
    POSTGRES_EVENT_EXISTS, POSTGRES_HISTORY_LOCK_ID, POSTGRES_MIGRATION_LOCK_ID,
    POSTGRES_MIGRATIONS_EXIST, POSTGRES_NAMESPACE, POSTGRES_PING, POSTGRES_RECORD_MIGRATION,
    POSTGRES_RETRYABLE_SQLSTATES, POSTGRES_RUN_BY_ID, POSTGRES_RUN_EXISTS, POSTGRES_SCHEMA,
    POSTGRES_SCHEMA_VERSION, POSTGRES_SESSION_TIMEOUT, POSTGRES_STATEMENT_TIMEOUT_MS,
    POSTGRES_TRANSACTION_RETRIES, POSTGRES_TRANSACTION_TIMEOUT, RUN_CURSOR_TIMESTAMP,
    SCHEMA_VERSION, UNKNOWN_EVENT_CURSOR, UNKNOWN_RUN_CURSOR,
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
use sqlbuild_state::execution_history::main::validate_page_limit::validate_page_limit;
use sqlbuild_state::execution_history::main::validate_postgres_settings::validate_postgres_settings;
use sqlbuild_state::execution_history::main::validate_schema_target::validate_schema_target;
use sqlbuild_state::execution_history::main::validate_stored_content::validate_stored_content;
use sqlbuild_state::execution_history::models::{
    Dialect, PostgresSettings, PreparedEvent, Query, RunCursorKey, RunRow, SqlParam,
};
use sqlbuild_state::execution_history::types::UtcTime;

use crate::bindings::_helpers::execution_history::appends::prepared_batch;
use crate::bindings::_helpers::execution_history::conversion::{
    checked, event_filter_spec, exact_int, history_error_of_kind, new_namespace, now,
    optional_text, parsed_status, projection_facts, rejection, run_filter_spec, run_records,
    run_rows, storage_error, stored_event, text_of, utc, utc_time,
};
use crate::bindings::_helpers::execution_history::statements::{
    column, execute, execute_plain, execute_query, fetch_all, fetch_one, tuple_of,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const PSYCOPG_MODULE: &str = "psycopg";
const PSYCOPG_ROWS_MODULE: &str = "psycopg.rows";
const CLOSED: &str = "PostgreSQL execution history storage is closed";
const MISSING_DRIVER: &str = "PostgreSQL execution history requires psycopg 3; install sqlbuild[postgres] or 'psycopg[binary]>=3.2'";
const CONNECT_FAILED: &str = "cannot connect to PostgreSQL execution history; verify the secret-resolved DSN, network access, and sqlbuild[postgres] installation";
const READ_FAILED: &str = "PostgreSQL execution history read failed";
const METADATA_INVALID: &str = "PostgreSQL execution history schema metadata is invalid";
const NAMESPACE_MISSING: &str = "PostgreSQL execution history cursor namespace is missing";
const STORAGE_ERROR: ExecutorHost = ExecutorHost::ExecutionHistoryStorageError;

/// The `psycopg.Error` class.
fn driver_error_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, PSYCOPG_MODULE, "Error")
}

fn driver_failure(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &driver_error_class(py)?))
}

/// Serialization failures and deadlocks retry while attempts remain.
fn retries_transaction(sqlstate: Option<&str>, attempts: i64, max_retries: i64) -> bool {
    sqlstate.is_some_and(|state| POSTGRES_RETRYABLE_SQLSTATES.contains(&state))
        && attempts < max_retries
}

/// `value` when it is an `int` that is not a `bool`.
fn storage_position(value: &Bound<'_, PyAny>, message: &str) -> PyResult<i64> {
    exact_int(value).ok_or_else(|| storage_error(value.py(), message))
}

/// `value.astimezone(UTC)` when it is a `datetime`.
fn utc_datetime<'py>(value: &Bound<'py, PyAny>, message: &str) -> PyResult<Bound<'py, PyAny>> {
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
fn stored_run(row: &Bound<'_, PyAny>) -> PyResult<RunRow> {
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

/// Deployed PostgreSQL implementation of event-log and run-projection contracts.
#[pyclass(
    module = "sqlbuild._native",
    name = "PostgresExecutionHistory",
    frozen,
    subclass,
    dict
)]
pub(crate) struct PostgresExecutionHistory {
    statement_timeout_ms: OnceLock<i64>,
    max_transaction_retries: OnceLock<i64>,
    closed: AtomicBool,
    project_calls: AtomicI64,
}

impl PostgresExecutionHistory {
    fn ensure_open(&self, py: Python<'_>) -> PyResult<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(storage_error(py, CLOSED));
        }
        Ok(())
    }

    /// Run `action` in a transaction with the statement timeout, retrying serialization failures.
    fn transactional<'py, T>(
        &self,
        slf: &Bound<'py, Self>,
        action: impl Fn(&Bound<'py, PyAny>) -> PyResult<T>,
        failure_message: &str,
    ) -> PyResult<T> {
        let py: Python<'py> = slf.py();
        let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
        let timeout: String = self
            .statement_timeout_ms
            .get()
            .copied()
            .unwrap_or(POSTGRES_STATEMENT_TIMEOUT_MS)
            .to_string();
        let max_retries: i64 = self
            .max_transaction_retries
            .get()
            .copied()
            .unwrap_or(POSTGRES_TRANSACTION_RETRIES);
        let mut attempts: i64 = 0;
        loop {
            let result: PyResult<T> = within(&connection.call_method0("transaction")?, |_| {
                within(&connection.call_method0("cursor")?, |cursor| {
                    execute(
                        cursor,
                        POSTGRES_TRANSACTION_TIMEOUT,
                        &[SqlParam::Text(timeout.clone())],
                    )?;
                    action(cursor)
                })
            });
            let error: PyErr = match result {
                Ok(value) => return Ok(value),
                Err(error) => error,
            };
            if history_error_of_kind(py, &error, STORAGE_ERROR)? || !driver_failure(py, &error)? {
                return Err(error);
            }
            let sqlstate: Option<String> = text_of(&native_field(error.value(py), "sqlstate")?)?;
            if retries_transaction(sqlstate.as_deref(), attempts, max_retries) {
                attempts += 1;
                continue;
            }
            return Err(storage_error(py, failure_message));
        }
    }

    /// `query` through a fresh cursor, with driver failures reported as read failures.
    fn read_rows<'py>(slf: &Bound<'py, Self>, query: &Query) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py: Python<'py> = slf.py();
        let connection: Bound<'py, PyAny> = native_field(slf, "_connection")?;
        let rows = within(&connection.call_method0("cursor")?, |cursor| {
            execute_query(cursor, query)?;
            fetch_all(cursor)
        });
        match rows {
            Err(error) if driver_failure(py, &error)? => Err(storage_error(py, READ_FAILED)),
            rows => rows,
        }
    }

    fn with_cursor<'py, T>(
        slf: &Bound<'py, Self>,
        cursor: Option<&Bound<'py, PyAny>>,
        body: impl FnOnce(&Bound<'py, PyAny>) -> PyResult<T>,
    ) -> PyResult<T> {
        if let Some(cursor) = cursor {
            return body(cursor);
        }
        let owned: Bound<'py, PyAny> = native_field(slf, "_connection")?.call_method0("cursor")?;
        let result: PyResult<T> = body(&owned);
        let closed = owned.call_method0("close");
        let value: T = result?;
        closed?;
        Ok(value)
    }

    fn schema_version_in<'py>(
        slf: &Bound<'py, Self>,
        cursor: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<i64> {
        let py: Python<'py> = slf.py();
        Self::with_cursor(slf, cursor, |active| {
            let read = (|| -> PyResult<i64> {
                execute_plain(active, POSTGRES_MIGRATIONS_EXIST)?;
                let Some(exists) = fetch_one(active)? else {
                    return Ok(0);
                };
                if exists.get_item("table_name")?.is_none() {
                    return Ok(0);
                }
                execute_plain(active, POSTGRES_SCHEMA_VERSION)?;
                let row = fetch_one(active)?;
                let value = row.map(|row| row.get_item("schema_version")).transpose()?;
                value
                    .as_ref()
                    .and_then(exact_int)
                    .ok_or_else(|| storage_error(py, METADATA_INVALID))
            })();
            match read {
                Err(error) if history_error_of_kind(py, &error, STORAGE_ERROR)? => Err(error),
                Err(error) if error.is_instance_of::<PyException>(py) => {
                    Err(storage_error(py, METADATA_INVALID))
                }
                read => read,
            }
        })
    }

    fn namespace_in<'py>(
        slf: &Bound<'py, Self>,
        cursor: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<String> {
        let py: Python<'py> = slf.py();
        Self::with_cursor(slf, cursor, |active| {
            execute_plain(active, POSTGRES_NAMESPACE)?;
            let row = fetch_one(active)?;
            let namespace = match row {
                Some(row) => text_of(&row.get_item("storage_namespace")?)?,
                None => None,
            };
            namespace.ok_or_else(|| storage_error(py, NAMESPACE_MISSING))
        })
    }

    fn stored_event<'py>(row: &Bound<'py, PyAny>, namespace: &str) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = row.py();
        let storage_id: i64 = storage_position(
            &column(row, "storage_id")?,
            "PostgreSQL event storage position is invalid",
        )?;
        let received_at = utc_datetime(
            &column(row, "received_at")?,
            "PostgreSQL event timestamp is invalid",
        )?;
        let payload: Bound<'py, PyAny> = column(row, "payload_json")?;
        if text_of(&payload)?.is_none() {
            return Err(storage_error(
                py,
                "PostgreSQL canonical event content is invalid",
            ));
        }
        stored_event(
            py,
            (
                storage_id,
                event_cursor(Dialect::Postgres, namespace, storage_id),
            ),
            received_at,
            &payload,
        )
    }

    fn stored_events<'py>(
        slf: &Bound<'py, Self>,
        rows: &[Bound<'py, PyAny>],
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let namespace: String = Self::namespace_in(slf, None)?;
        rows.iter()
            .map(|row| Self::stored_event(row, &namespace))
            .collect()
    }

    fn read_all_runs(cursor: &Bound<'_, PyAny>) -> PyResult<Vec<RunRow>> {
        execute_plain(cursor, POSTGRES_ALL_RUNS)?;
        fetch_all(cursor)?.iter().map(stored_run).collect()
    }

    fn lock_history(cursor: &Bound<'_, PyAny>) -> PyResult<()> {
        execute(
            cursor,
            POSTGRES_ADVISORY_LOCK,
            &[SqlParam::Integer(POSTGRES_HISTORY_LOCK_ID)],
        )?;
        Ok(())
    }

    fn append_via<'py>(
        slf: &Bound<'py, Self>,
        cursor: &Bound<'py, PyAny>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        native_field(slf, "_append_events")?.call(
            (),
            Some(&keywords(
                py,
                &[("cursor", cursor.clone()), ("events", events.clone())],
            )?),
        )
    }

    fn replace_via<'py>(
        slf: &Bound<'py, Self>,
        cursor: &Bound<'py, PyAny>,
        projected: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py: Python<'py> = slf.py();
        native_field(slf, "_replace_projection")?.call(
            (),
            Some(&keywords(
                py,
                &[("cursor", cursor.clone()), ("projected", projected.clone())],
            )?),
        )?;
        Ok(())
    }

    fn migrate(&self, slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        self.transactional(
            slf,
            |cursor| {
                execute(
                    cursor,
                    POSTGRES_ADVISORY_LOCK,
                    &[SqlParam::Integer(POSTGRES_MIGRATION_LOCK_ID)],
                )?;
                let current: i64 = Self::schema_version_in(slf, Some(cursor))?;
                if !checked(py, schema_migration_needed(Dialect::Postgres, current))? {
                    return Ok(());
                }
                for statement in POSTGRES_SCHEMA {
                    execute_plain(cursor, statement)?;
                }
                execute(
                    cursor,
                    POSTGRES_RECORD_MIGRATION,
                    &[
                        SqlParam::Integer(SCHEMA_VERSION),
                        SqlParam::Text(new_namespace(py)?),
                    ],
                )?;
                Ok(())
            },
            "PostgreSQL execution history schema migration failed",
        )
    }

    fn run_cursor_key(
        slf: &Bound<'_, Self>,
        cursor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<RunCursorKey>> {
        let py: Python<'_> = slf.py();
        let Some(cursor) = cursor.filter(|cursor| !cursor.is_none()) else {
            return Ok(None);
        };
        let text: String = text_of(cursor)?.unwrap_or_default();
        let namespace: String = Self::namespace_in(slf, None)?;
        let decoded = checked(py, decoded_run_cursor(Dialect::Postgres, &text, &namespace))?;
        let invalid = || rejection(py, HistoryErrorKind::InvalidCursor, RUN_CURSOR_TIMESTAMP);
        let parsed = imported(py, "datetime", "datetime")?
            .call_method1("fromisoformat", (decoded.created_at_text.as_str(),));
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => {
                return Err(invalid());
            }
            Err(error) => return Err(error),
        };
        if parsed.call_method0("utcoffset")?.is_none() {
            return Err(invalid());
        }
        let created_at = utc_time(&parsed)?;
        let rows = Self::read_rows(
            slf,
            &Query {
                sql: POSTGRES_RUN_EXISTS.to_owned(),
                params: vec![
                    SqlParam::Timestamp(created_at),
                    SqlParam::Text(decoded.run_id.clone()),
                ],
            },
        )?;
        if rows.is_empty() {
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
impl PostgresExecutionHistory {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        Self {
            statement_timeout_ms: OnceLock::new(),
            max_transaction_retries: OnceLock::new(),
            closed: AtomicBool::new(true),
            project_calls: AtomicI64::new(0),
        }
    }

    #[pyo3(signature = (dsn, **options))]
    fn __init__(
        slf: &Bound<'_, Self>,
        dsn: &Bound<'_, PyAny>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        let options = KeywordArguments::read(
            py,
            "__init__",
            options,
            (
                &[],
                &[
                    "connect_timeout_seconds",
                    "statement_timeout_ms",
                    "max_transaction_retries",
                ],
            ),
        )?;
        let setting = |name: &str, default: i64| -> PyResult<Option<i64>> {
            if options.passed(name)? {
                return Ok(exact_int(&options.value(name)?));
            }
            Ok(Some(default))
        };
        let settings = PostgresSettings {
            dsn: text_of(dsn)?,
            connect_timeout_seconds: setting(
                "connect_timeout_seconds",
                POSTGRES_CONNECT_TIMEOUT_SECONDS,
            )?,
            statement_timeout_ms: setting("statement_timeout_ms", POSTGRES_STATEMENT_TIMEOUT_MS)?,
            max_transaction_retries: setting(
                "max_transaction_retries",
                POSTGRES_TRANSACTION_RETRIES,
            )?,
        };
        checked(py, validate_postgres_settings(&settings))?;
        let (psycopg, dict_row) = match (
            py.import(PSYCOPG_MODULE),
            imported(py, PSYCOPG_ROWS_MODULE, "dict_row"),
        ) {
            (Ok(psycopg), Ok(dict_row)) => (psycopg, dict_row),
            (Err(error), _) | (_, Err(error))
                if error.is_instance_of::<pyo3::exceptions::PyImportError>(py) =>
            {
                return Err(storage_error(py, MISSING_DRIVER));
            }
            (Err(error), _) | (_, Err(error)) => return Err(error),
        };
        let statement_timeout: i64 = settings
            .statement_timeout_ms
            .unwrap_or(POSTGRES_STATEMENT_TIMEOUT_MS);
        let _ = this.max_transaction_retries.set(
            settings
                .max_transaction_retries
                .unwrap_or(POSTGRES_TRANSACTION_RETRIES),
        );
        this.closed.store(false, Ordering::SeqCst);
        let mut connection: Option<Bound<'_, PyAny>> = None;
        let opened: PyResult<()> = (|| {
            let opened: Bound<'_, PyAny> = native_field(&psycopg, "connect")?.call(
                (dsn,),
                Some(&keywords(
                    py,
                    &[
                        (
                            "connect_timeout",
                            settings.connect_timeout_seconds.into_bound_py_any(py)?,
                        ),
                        ("autocommit", PyBool::new(py, true).to_owned().into_any()),
                        ("row_factory", dict_row.clone()),
                    ],
                )?),
            )?;
            connection = Some(opened.clone());
            slf.setattr("_connection", &opened)?;
            within(&opened.call_method0("cursor")?, |cursor| {
                execute(
                    cursor,
                    POSTGRES_SESSION_TIMEOUT,
                    &[SqlParam::Text(statement_timeout.to_string())],
                )
            })?;
            let _ = this.statement_timeout_ms.set(statement_timeout);
            this.migrate(slf)?;
            slf.call_method0("reconcile")?;
            Ok(())
        })();
        let Err(error) = opened else {
            return Ok(());
        };
        if !error.is_instance_of::<PyException>(py) {
            return Err(error);
        }
        if let Some(connection) = connection {
            let _ = connection.call_method0("close");
        }
        this.closed.store(true, Ordering::SeqCst);
        if history_error_of_kind(py, &error, STORAGE_ERROR)? {
            return Err(error);
        }
        Err(storage_error(py, CONNECT_FAILED))
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let state: &str = if slf.get().closed.load(Ordering::SeqCst) {
            "closed"
        } else {
            "open"
        };
        let name: String = slf.get_type().getattr("__name__")?.extract()?;
        Ok(format!("{name}(state='{state}')"))
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
        slf.get().ensure_open(py)?;
        slf.call_method1("append_events", (tuple_of(py, vec![event.clone()])?,))?
            .get_item(0)
    }

    fn append_events<'py>(
        slf: &Bound<'py, Self>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.ensure_open(py)?;
        let pending: Bound<'py, PyTuple> =
            PyTuple::new(py, events.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        this.transactional(
            slf,
            |cursor| {
                Self::lock_history(cursor)?;
                Self::append_via(slf, cursor, pending.as_any())
            },
            "PostgreSQL event append failed",
        )
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
        slf.get().ensure_open(py)?;
        let limit: Option<i64> = limit.map_or(Some(DEFAULT_PAGE_LIMIT), exact_int);
        checked(py, validate_page_limit(limit))?;
        let limit: i64 = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
        let after: i64 = match after_cursor.filter(|cursor| !cursor.is_none()) {
            None => 0,
            Some(cursor) => {
                let text: String = text_of(cursor)?.unwrap_or_default();
                let namespace: String = Self::namespace_in(slf, None)?;
                let position: i64 = checked(
                    py,
                    event_cursor_position(Dialect::Postgres, Some(&text), &namespace),
                )?;
                let rows = Self::read_rows(
                    slf,
                    &Query {
                        sql: POSTGRES_EVENT_EXISTS.to_owned(),
                        params: vec![SqlParam::Integer(position)],
                    },
                )?;
                if rows.is_empty() {
                    return Err(rejection(
                        py,
                        HistoryErrorKind::InvalidCursor,
                        UNKNOWN_EVENT_CURSOR,
                    ));
                }
                position
            }
        };
        let spec = event_filter_spec(event_filter)?;
        let rows = Self::read_rows(
            slf,
            &event_page_query(Dialect::Postgres, after, &spec, limit),
        )?;
        let has_more: bool = rows.len() as i64 > limit;
        let page: Vec<Bound<'py, PyAny>> = rows.into_iter().take(limit as usize).collect();
        let records: Vec<Bound<'py, PyAny>> = Self::stored_events(slf, &page)?;
        let next_cursor: Bound<'py, PyAny> = match records.last() {
            Some(record) => native_field(record, "cursor")?,
            None => py.None().into_bound(py),
        };
        call_with(
            &host_entry(py, ExecutorHost::EventPage)?,
            &[
                ("records", tuple_of(py, records)?.into_any()),
                ("next_cursor", next_cursor),
                ("has_more", has_more.into_bound_py_any(py)?),
            ],
        )
    }

    #[pyo3(name = "get_run")]
    fn run_record<'py>(
        slf: &Bound<'py, Self>,
        run_id: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        slf.get().ensure_open(py)?;
        let run_id: Option<String> = optional_text(run_id)?;
        let params: Vec<SqlParam> = vec![run_id.map_or(SqlParam::Null, SqlParam::Text)];
        let rows = Self::read_rows(
            slf,
            &Query {
                sql: POSTGRES_RUN_BY_ID.to_owned(),
                params,
            },
        )?;
        match rows.first() {
            Some(row) => crate::bindings::_helpers::execution_history::conversion::run_record(
                py,
                &stored_run(row)?,
            ),
            None => Ok(py.None().into_bound(py)),
        }
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
        slf.get().ensure_open(py)?;
        let limit: Option<i64> = limit.map_or(Some(DEFAULT_PAGE_LIMIT), exact_int);
        checked(py, validate_page_limit(limit))?;
        let limit: i64 = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
        let after = Self::run_cursor_key(slf, after_cursor)?;
        let spec = run_filter_spec(run_filter)?;
        let rows = Self::read_rows(
            slf,
            &run_page_query(Dialect::Postgres, after.as_ref(), &spec, limit),
        )?;
        let runs: Vec<RunRow> = rows
            .iter()
            .take(limit as usize)
            .map(stored_run)
            .collect::<PyResult<_>>()?;
        let next_cursor: Bound<'py, PyAny> = match runs.last() {
            Some(run) => {
                let namespace: String = Self::namespace_in(slf, None)?;
                checked(py, run_cursor(Dialect::Postgres, &namespace, run))?
                    .into_bound_py_any(py)?
            }
            None => py.None().into_bound(py),
        };
        call_with(
            &host_entry(py, ExecutorHost::RunPage)?,
            &[
                ("records", run_records(py, &runs)?.into_any()),
                ("next_cursor", next_cursor),
                (
                    "has_more",
                    (rows.len() as i64 > limit).into_bound_py_any(py)?,
                ),
            ],
        )
    }

    fn project<'py>(
        slf: &Bound<'py, Self>,
        stored_events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.ensure_open(py)?;
        let pending: Bound<'py, PyTuple> =
            PyTuple::new(py, stored_events.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        this.project_calls.fetch_add(1, Ordering::SeqCst);
        this.transactional(
            slf,
            |cursor| {
                Self::lock_history(cursor)?;
                let facts = projection_facts(pending.as_any())?;
                let projected = checked(py, project_runs(facts, Self::read_all_runs(cursor)?))?;
                let records: Bound<'py, PyTuple> = run_records(py, &projected)?;
                Self::replace_via(slf, cursor, records.as_any())?;
                Ok(records)
            },
            "PostgreSQL run projection publication failed",
        )
    }

    fn rebuild_from_events<'py>(
        slf: &Bound<'py, Self>,
        stored_events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.ensure_open(py)?;
        let pending: Bound<'py, PyTuple> =
            PyTuple::new(py, stored_events.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        this.transactional(
            slf,
            |cursor| {
                Self::lock_history(cursor)?;
                let projected = checked(
                    py,
                    project_runs(projection_facts(pending.as_any())?, Vec::new()),
                )?;
                let records: Bound<'py, PyTuple> = run_records(py, &projected)?;
                Self::replace_via(slf, cursor, records.as_any())?;
                Ok(records)
            },
            "PostgreSQL run projection rebuild failed",
        )
    }

    /// Atomically append immutable facts and publish their run projection.
    fn append_and_project<'py>(
        slf: &Bound<'py, Self>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.ensure_open(py)?;
        let pending: Bound<'py, PyTuple> =
            PyTuple::new(py, events.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        this.transactional(
            slf,
            |cursor| {
                Self::lock_history(cursor)?;
                let stored: Bound<'py, PyAny> = Self::append_via(slf, cursor, pending.as_any())?;
                let projected = checked(
                    py,
                    project_runs(projection_facts(&stored)?, Self::read_all_runs(cursor)?),
                )?;
                let records: Bound<'py, PyTuple> = run_records(py, &projected)?;
                Self::replace_via(slf, cursor, records.as_any())?;
                Ok(stored)
            },
            "PostgreSQL atomic append and projection failed",
        )
    }

    /// Rebuild the disposable run projection from all durable event facts.
    fn reconcile<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.ensure_open(py)?;
        this.transactional(
            slf,
            |cursor| {
                Self::lock_history(cursor)?;
                let stored: Bound<'py, PyAny> = native_field(slf, "_read_all_events")?
                    .call((), Some(&keywords(py, &[("cursor", cursor.clone())])?))?;
                let projected = checked(py, project_runs(projection_facts(&stored)?, Vec::new()))?;
                let records: Bound<'py, PyTuple> = run_records(py, &projected)?;
                Self::replace_via(slf, cursor, records.as_any())?;
                Ok(records)
            },
            "PostgreSQL reconciliation failed",
        )
    }

    /// Verify connectivity and the supported schema revision.
    fn check_health(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let py: Python<'_> = slf.py();
        slf.get().ensure_open(py)?;
        let healthy = (|| -> PyResult<bool> {
            let connection: Bound<'_, PyAny> = native_field(slf, "_connection")?;
            let row = within(&connection.call_method0("cursor")?, |cursor| {
                execute_plain(cursor, POSTGRES_PING)?;
                fetch_one(cursor)
            })?;
            let version: i64 = slf.call_method0("get_schema_version")?.extract()?;
            Ok(row.is_some() && version == SCHEMA_VERSION)
        })();
        match healthy {
            Err(error) if driver_failure(py, &error)? => Ok(false),
            healthy => healthy,
        }
    }

    #[pyo3(name = "get_schema_version")]
    fn schema_version(slf: &Bound<'_, Self>) -> PyResult<i64> {
        slf.get().ensure_open(slf.py())?;
        Self::schema_version_in(slf, None)
    }

    #[pyo3(signature = (*, target_version = None))]
    fn upgrade_schema(slf: &Bound<'_, Self>, target_version: Option<i64>) -> PyResult<i64> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.ensure_open(py)?;
        let target: i64 = target_version.unwrap_or(SCHEMA_VERSION);
        checked(py, validate_schema_target(Dialect::Postgres, target))?;
        this.migrate(slf)?;
        Ok(target)
    }

    fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        let this: &Self = slf.get();
        if this.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        native_field(slf, "_connection")?.call_method0("close")?;
        this.closed.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn dispose(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.call_method0("close")?;
        Ok(())
    }

    fn __enter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        slf.get().ensure_open(slf.py())?;
        Ok(slf.clone())
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(slf: &Bound<'_, Self>, _exc_info: &Bound<'_, PyTuple>) -> PyResult<Option<bool>> {
        slf.call_method0("close")?;
        Ok(None)
    }

    #[pyo3(signature = (*, cursor, events))]
    fn _append_events<'py>(
        slf: &Bound<'py, Self>,
        cursor: &Bound<'py, PyAny>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        let prepared: Vec<PreparedEvent> = prepared_batch(events)?;
        let mut stored: Vec<Bound<'py, PyAny>> = Vec::with_capacity(prepared.len());
        let mut namespace: Option<String> = None;
        let received_at = now(py)?;
        for event in &prepared {
            execute_query(
                cursor,
                &event_insert_query(Dialect::Postgres, event, &received_at),
            )?;
            let mut row = fetch_one(cursor)?;
            if row.is_none() {
                execute(
                    cursor,
                    POSTGRES_EVENT_BY_ID,
                    &[SqlParam::Text(event.event_id.clone())],
                )?;
                row = fetch_one(cursor)?;
            }
            let Some(row) = row else {
                return Err(storage_error(
                    py,
                    "PostgreSQL event append did not produce a durable fact",
                ));
            };
            let digest: Option<String> = text_of(&column(&row, "content_digest")?)?;
            let payload: Option<String> = text_of(&column(&row, "payload_json")?)?;
            checked(
                py,
                validate_stored_content(event, (digest.as_deref(), payload.as_deref())),
            )?;
            if namespace.is_none() {
                namespace = Some(Self::namespace_in(slf, Some(cursor))?);
            }
            stored.push(Self::stored_event(
                &row,
                namespace.as_deref().unwrap_or_default(),
            )?);
        }
        tuple_of(py, stored)
    }

    #[pyo3(signature = (*, cursor, projected))]
    fn _replace_projection(
        _slf: &Bound<'_, Self>,
        cursor: &Bound<'_, PyAny>,
        projected: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        execute_plain(cursor, POSTGRES_DELETE_RUNS)?;
        for run in run_rows(projected)? {
            execute_query(cursor, &run_insert_query(Dialect::Postgres, &run))?;
        }
        Ok(())
    }

    #[pyo3(signature = (*, cursor))]
    fn _read_all_events<'py>(
        slf: &Bound<'py, Self>,
        cursor: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = slf.py();
        execute_plain(cursor, POSTGRES_ALL_EVENTS)?;
        let rows: Vec<Bound<'py, PyAny>> = fetch_all(cursor)?;
        let stored: Vec<Bound<'py, PyAny>> = if rows.is_empty() {
            Vec::new()
        } else {
            let namespace: String = Self::namespace_in(slf, Some(cursor))?;
            rows.iter()
                .map(|row| Self::stored_event(row, &namespace))
                .collect::<PyResult<_>>()?
        };
        tuple_of(py, stored)
    }
}

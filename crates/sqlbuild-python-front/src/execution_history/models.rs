//! Public records of execution history.

use crate::execution_history::_helpers::appends::prepared_batch;
use crate::execution_history::_helpers::conversion::checked;
use crate::execution_history::_helpers::conversion::event_filter_spec;
use crate::execution_history::_helpers::conversion::exact_int;
use crate::execution_history::_helpers::conversion::history_error_of_kind;
use crate::execution_history::_helpers::conversion::new_namespace;
use crate::execution_history::_helpers::conversion::now;
use crate::execution_history::_helpers::conversion::projection_facts;
use crate::execution_history::_helpers::conversion::python_params;
use crate::execution_history::_helpers::conversion::rejection;
use crate::execution_history::_helpers::conversion::run_filter_spec;
use crate::execution_history::_helpers::conversion::run_records;
use crate::execution_history::_helpers::conversion::run_rows;
use crate::execution_history::_helpers::conversion::storage_error;
use crate::execution_history::_helpers::conversion::storage_error_from;
use crate::execution_history::_helpers::conversion::stored_event;
use crate::execution_history::_helpers::conversion::utc_time;
use crate::execution_history::_helpers::postgres::CONNECT_FAILED;
use crate::execution_history::_helpers::postgres::METADATA_INVALID;
use crate::execution_history::_helpers::postgres::MISSING_DRIVER;
use crate::execution_history::_helpers::postgres::NAMESPACE_MISSING;
use crate::execution_history::_helpers::postgres::PSYCOPG_MODULE;
use crate::execution_history::_helpers::postgres::PSYCOPG_ROWS_MODULE;
use crate::execution_history::_helpers::postgres::READ_FAILED;
use crate::execution_history::_helpers::postgres::driver_failure;
use crate::execution_history::_helpers::postgres::retries_transaction;
use crate::execution_history::_helpers::postgres::storage_position;
use crate::execution_history::_helpers::postgres::utc_datetime;
use crate::execution_history::_helpers::sqlite::INTEGRITY_CONFLICT;
use crate::execution_history::_helpers::sqlite::PATHLIB_MODULE;
use crate::execution_history::_helpers::sqlite::parsed_timestamp;
use crate::execution_history::_helpers::sqlite::prepare_history_directory;
use crate::execution_history::_helpers::sqlite::restrict_history_file;
use crate::execution_history::_helpers::sqlite::sqlite_failure;
use crate::execution_history::_helpers::sqlite::wrapped;
use crate::execution_history::_helpers::statements::column;
use crate::execution_history::_helpers::statements::execute;
use crate::execution_history::_helpers::statements::execute_plain;
use crate::execution_history::_helpers::statements::execute_query;
use crate::execution_history::_helpers::statements::fetch_all;
use crate::execution_history::_helpers::statements::tuple_of;
use crate::execution_history::main::fetch_one::fetch_one;
use crate::execution_history::main::nullable_text::nullable_text;
use crate::execution_history::main::parsed_iso::parsed_iso;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyException;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::types::PyBool;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::optional_text::optional_text;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_threads::models::ReentrantLock;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::warehouse_access::main::sqlite_params::sqlite_params;
use sqlbuild_python_base::warehouse_access::models::SqliteConnection;
use sqlbuild_python_base::warehouse_access::models::SqliteCursor;
use sqlbuild_state::execution_history::constants::DEFAULT_PAGE_LIMIT;
use sqlbuild_state::execution_history::constants::MEMORY_PATH;
use sqlbuild_state::execution_history::constants::POSTGRES_ADVISORY_LOCK;
use sqlbuild_state::execution_history::constants::POSTGRES_ALL_EVENTS;
use sqlbuild_state::execution_history::constants::POSTGRES_ALL_RUNS;
use sqlbuild_state::execution_history::constants::POSTGRES_CONNECT_TIMEOUT_SECONDS;
use sqlbuild_state::execution_history::constants::POSTGRES_DELETE_RUNS;
use sqlbuild_state::execution_history::constants::POSTGRES_EVENT_BY_ID;
use sqlbuild_state::execution_history::constants::POSTGRES_EVENT_EXISTS;
use sqlbuild_state::execution_history::constants::POSTGRES_HISTORY_LOCK_ID;
use sqlbuild_state::execution_history::constants::POSTGRES_MIGRATION_LOCK_ID;
use sqlbuild_state::execution_history::constants::POSTGRES_MIGRATIONS_EXIST;
use sqlbuild_state::execution_history::constants::POSTGRES_NAMESPACE;
use sqlbuild_state::execution_history::constants::POSTGRES_PING;
use sqlbuild_state::execution_history::constants::POSTGRES_RECORD_MIGRATION;
use sqlbuild_state::execution_history::constants::POSTGRES_RUN_BY_ID;
use sqlbuild_state::execution_history::constants::POSTGRES_RUN_EXISTS;
use sqlbuild_state::execution_history::constants::POSTGRES_SCHEMA;
use sqlbuild_state::execution_history::constants::POSTGRES_SCHEMA_VERSION;
use sqlbuild_state::execution_history::constants::POSTGRES_SESSION_TIMEOUT;
use sqlbuild_state::execution_history::constants::POSTGRES_STATEMENT_TIMEOUT_MS;
use sqlbuild_state::execution_history::constants::POSTGRES_TRANSACTION_RETRIES;
use sqlbuild_state::execution_history::constants::POSTGRES_TRANSACTION_TIMEOUT;
use sqlbuild_state::execution_history::constants::RUN_CURSOR_TIMESTAMP;
use sqlbuild_state::execution_history::constants::SCHEMA_VERSION;
use sqlbuild_state::execution_history::constants::SQLITE_ALL_EVENTS;
use sqlbuild_state::execution_history::constants::SQLITE_ALL_RUNS;
use sqlbuild_state::execution_history::constants::SQLITE_BEGIN;
use sqlbuild_state::execution_history::constants::SQLITE_BUSY_TIMEOUT_MS;
use sqlbuild_state::execution_history::constants::SQLITE_COMMIT;
use sqlbuild_state::execution_history::constants::SQLITE_DELETE_RUNS;
use sqlbuild_state::execution_history::constants::SQLITE_EVENT_BY_ID;
use sqlbuild_state::execution_history::constants::SQLITE_EVENT_BY_STORAGE_ID;
use sqlbuild_state::execution_history::constants::SQLITE_EVENT_EXISTS;
use sqlbuild_state::execution_history::constants::SQLITE_FOREIGN_KEYS;
use sqlbuild_state::execution_history::constants::SQLITE_HEALTHY_CHECK_RESULT;
use sqlbuild_state::execution_history::constants::SQLITE_INSERT_NAMESPACE;
use sqlbuild_state::execution_history::constants::SQLITE_INSERT_SCHEMA_VERSION;
use sqlbuild_state::execution_history::constants::SQLITE_JOURNAL_MODE;
use sqlbuild_state::execution_history::constants::SQLITE_METADATA_EXISTS;
use sqlbuild_state::execution_history::constants::SQLITE_METADATA_TABLE;
use sqlbuild_state::execution_history::constants::SQLITE_METADATA_VALUE;
use sqlbuild_state::execution_history::constants::SQLITE_NAMESPACE_KEY;
use sqlbuild_state::execution_history::constants::SQLITE_QUICK_CHECK;
use sqlbuild_state::execution_history::constants::SQLITE_ROLLBACK;
use sqlbuild_state::execution_history::constants::SQLITE_RUN_BY_ID;
use sqlbuild_state::execution_history::constants::SQLITE_RUN_EXISTS;
use sqlbuild_state::execution_history::constants::SQLITE_RUN_INSERT;
use sqlbuild_state::execution_history::constants::SQLITE_SCHEMA;
use sqlbuild_state::execution_history::constants::SQLITE_SCHEMA_VERSION_KEY;
use sqlbuild_state::execution_history::constants::UNKNOWN_EVENT_CURSOR;
use sqlbuild_state::execution_history::constants::UNKNOWN_RUN_CURSOR;
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
use sqlbuild_state::execution_history::main::validate_postgres_settings::validate_postgres_settings;
use sqlbuild_state::execution_history::main::validate_schema_target::validate_schema_target;
use sqlbuild_state::execution_history::main::validate_stored_content::validate_stored_content;
use sqlbuild_state::execution_history::models::Dialect;
use sqlbuild_state::execution_history::models::PostgresSettings;
use sqlbuild_state::execution_history::models::PreparedEvent;
use sqlbuild_state::execution_history::models::Query;
use sqlbuild_state::execution_history::models::RunCursorKey;
use sqlbuild_state::execution_history::models::RunRow;
use sqlbuild_state::execution_history::models::SqlParam;
use sqlbuild_state::execution_history::models::SqliteLocation;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

/// Deployed PostgreSQL implementation of event-log and run-projection contracts.
#[pyclass(
    module = "sqlbuild._native",
    name = "PostgresExecutionHistory",
    frozen,
    subclass,
    dict
)]
#[derive(Debug)]
pub struct PostgresExecutionHistory {
    pub(crate) statement_timeout_ms: OnceLock<i64>,
    pub(crate) max_transaction_retries: OnceLock<i64>,
    pub(crate) closed: AtomicBool,
    pub(crate) project_calls: AtomicI64,
}

impl PostgresExecutionHistory {
    pub(crate) fn ensure_open(&self, py: Python<'_>) -> PyResult<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(storage_error(
                py,
                crate::execution_history::_helpers::postgres::CLOSED,
            ));
        }
        Ok(())
    }

    /// Run `action` in a transaction with the statement timeout, retrying serialization failures.
    pub(crate) fn transactional<'py, T>(
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
            if history_error_of_kind(
                py,
                &error,
                crate::execution_history::_helpers::postgres::STORAGE_ERROR,
            )? || !driver_failure(py, &error)?
            {
                return Err(error);
            }
            let sqlstate: Option<String> =
                optional_text(&native_field(error.value(py), "sqlstate")?)?;
            if retries_transaction(sqlstate.as_deref(), attempts, max_retries) {
                attempts += 1;
                continue;
            }
            return Err(storage_error(py, failure_message));
        }
    }

    /// `query` through a fresh cursor, with driver failures reported as read failures.
    pub(crate) fn read_rows<'py>(
        slf: &Bound<'py, Self>,
        query: &Query,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
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

    pub(crate) fn with_cursor<'py, T>(
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

    pub(crate) fn schema_version_in<'py>(
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
                Err(error)
                    if history_error_of_kind(
                        py,
                        &error,
                        crate::execution_history::_helpers::postgres::STORAGE_ERROR,
                    )? =>
                {
                    Err(error)
                }
                Err(error) if error.is_instance_of::<PyException>(py) => {
                    Err(storage_error(py, METADATA_INVALID))
                }
                read => read,
            }
        })
    }

    pub(crate) fn namespace_in<'py>(
        slf: &Bound<'py, Self>,
        cursor: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<String> {
        let py: Python<'py> = slf.py();
        Self::with_cursor(slf, cursor, |active| {
            execute_plain(active, POSTGRES_NAMESPACE)?;
            let row = fetch_one(active)?;
            let namespace = match row {
                Some(row) => optional_text(&row.get_item("storage_namespace")?)?,
                None => None,
            };
            namespace.ok_or_else(|| storage_error(py, NAMESPACE_MISSING))
        })
    }

    pub(crate) fn stored_event<'py>(
        row: &Bound<'py, PyAny>,
        namespace: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
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
        if optional_text(&payload)?.is_none() {
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

    pub(crate) fn stored_events<'py>(
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

    pub(crate) fn read_all_runs(cursor: &Bound<'_, PyAny>) -> PyResult<Vec<RunRow>> {
        execute_plain(cursor, POSTGRES_ALL_RUNS)?;
        fetch_all(cursor)?
            .iter()
            .map(crate::execution_history::_helpers::postgres::stored_run)
            .collect()
    }

    pub(crate) fn lock_history(cursor: &Bound<'_, PyAny>) -> PyResult<()> {
        execute(
            cursor,
            POSTGRES_ADVISORY_LOCK,
            &[SqlParam::Integer(POSTGRES_HISTORY_LOCK_ID)],
        )?;
        Ok(())
    }

    pub(crate) fn append_via<'py>(
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

    pub(crate) fn replace_via<'py>(
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

    pub(crate) fn migrate(&self, slf: &Bound<'_, Self>) -> PyResult<()> {
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

    pub(crate) fn run_cursor_key(
        slf: &Bound<'_, Self>,
        cursor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<RunCursorKey>> {
        let py: Python<'_> = slf.py();
        let Some(cursor) = cursor.filter(|cursor| !cursor.is_none()) else {
            return Ok(None);
        };
        let text: String = optional_text(cursor)?.unwrap_or_default();
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
    pub(crate) fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        Self {
            statement_timeout_ms: OnceLock::new(),
            max_transaction_retries: OnceLock::new(),
            closed: AtomicBool::new(true),
            project_calls: AtomicI64::new(0),
        }
    }

    #[pyo3(signature = (dsn, **options))]
    pub(crate) fn __init__(
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
            dsn: optional_text(dsn)?,
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
        if history_error_of_kind(
            py,
            &error,
            crate::execution_history::_helpers::postgres::STORAGE_ERROR,
        )? {
            return Err(error);
        }
        Err(storage_error(py, CONNECT_FAILED))
    }

    pub(crate) fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
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
    pub fn project_calls(&self) -> i64 {
        self.project_calls.load(Ordering::SeqCst)
    }

    pub(crate) fn append_event<'py>(
        slf: &Bound<'py, Self>,
        event: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        slf.get().ensure_open(py)?;
        slf.call_method1("append_events", (tuple_of(py, vec![event.clone()])?,))?
            .get_item(0)
    }

    pub(crate) fn append_events<'py>(
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
    pub(crate) fn stored_events_page<'py>(
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
                let text: String = optional_text(cursor)?.unwrap_or_default();
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
    pub(crate) fn run_record<'py>(
        slf: &Bound<'py, Self>,
        run_id: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        slf.get().ensure_open(py)?;
        let run_id: Option<String> = nullable_text(run_id)?;
        let params: Vec<SqlParam> = vec![run_id.map_or(SqlParam::Null, SqlParam::Text)];
        let rows = Self::read_rows(
            slf,
            &Query {
                sql: POSTGRES_RUN_BY_ID.to_owned(),
                params,
            },
        )?;
        match rows.first() {
            Some(row) => crate::execution_history::_helpers::conversion::run_record(
                py,
                &crate::execution_history::_helpers::postgres::stored_run(row)?,
            ),
            None => Ok(py.None().into_bound(py)),
        }
    }

    #[pyo3(signature = (*, run_filter, after_cursor = None, limit = None))]
    #[pyo3(name = "get_runs")]
    pub(crate) fn run_page<'py>(
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
            .map(crate::execution_history::_helpers::postgres::stored_run)
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

    pub(crate) fn project<'py>(
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

    pub(crate) fn rebuild_from_events<'py>(
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
    pub(crate) fn append_and_project<'py>(
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
    pub(crate) fn reconcile<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
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
    pub(crate) fn check_health(slf: &Bound<'_, Self>) -> PyResult<bool> {
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
    pub(crate) fn schema_version(slf: &Bound<'_, Self>) -> PyResult<i64> {
        slf.get().ensure_open(slf.py())?;
        Self::schema_version_in(slf, None)
    }

    #[pyo3(signature = (*, target_version = None))]
    pub(crate) fn upgrade_schema(
        slf: &Bound<'_, Self>,
        target_version: Option<i64>,
    ) -> PyResult<i64> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.ensure_open(py)?;
        let target: i64 = target_version.unwrap_or(SCHEMA_VERSION);
        checked(py, validate_schema_target(Dialect::Postgres, target))?;
        this.migrate(slf)?;
        Ok(target)
    }

    pub(crate) fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        let this: &Self = slf.get();
        if this.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        native_field(slf, "_connection")?.call_method0("close")?;
        this.closed.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn dispose(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.call_method0("close")?;
        Ok(())
    }

    pub(crate) fn __enter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        slf.get().ensure_open(slf.py())?;
        Ok(slf.clone())
    }

    #[pyo3(signature = (*_exc_info))]
    pub(crate) fn __exit__(
        slf: &Bound<'_, Self>,
        _exc_info: &Bound<'_, PyTuple>,
    ) -> PyResult<Option<bool>> {
        slf.call_method0("close")?;
        Ok(None)
    }

    #[pyo3(signature = (*, cursor, events))]
    pub(crate) fn _append_events<'py>(
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
            let digest: Option<String> = optional_text(&column(&row, "content_digest")?)?;
            let payload: Option<String> = optional_text(&column(&row, "payload_json")?)?;
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
    pub(crate) fn _replace_projection(
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
    pub(crate) fn _read_all_events<'py>(
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

/// Project-local SQLite implementation of event-log and run-projection storage contracts.
#[pyclass(
    module = "sqlbuild._native",
    name = "SQLiteExecutionHistory",
    frozen,
    subclass,
    dict
)]
#[derive(Debug)]
pub struct SqliteExecutionHistory {
    pub(crate) path: OnceLock<String>,
    pub(crate) lock: ReentrantLock,
    pub(crate) closed: AtomicBool,
    pub(crate) project_calls: AtomicI64,
}

impl SqliteExecutionHistory {
    pub(crate) fn database_path(&self) -> &str {
        self.path.get().map_or(MEMORY_PATH, String::as_str)
    }

    pub(crate) fn with_lock<'py, T>(
        &self,
        py: Python<'py>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        self.lock.with_lock(py, body)
    }

    pub(crate) fn ensure_open(&self, py: Python<'_>) -> PyResult<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(storage_error(
                py,
                crate::execution_history::_helpers::sqlite::CLOSED,
            ));
        }
        Ok(())
    }

    /// Run `body` in `BEGIN IMMEDIATE`, committing on success and rolling back on failure.
    pub(crate) fn transaction<'py, T>(
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

    pub(crate) fn metadata_value<'py>(
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

    pub(crate) fn metadata_version(connection: &Bound<'_, PyAny>) -> PyResult<i64> {
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
        match optional_text(&value)? {
            Some(text) => text.trim().parse::<i64>().map_err(|_| invalid()),
            None => exact_int(&value).ok_or_else(invalid),
        }
    }

    pub(crate) fn namespace(connection: &Bound<'_, PyAny>) -> PyResult<String> {
        let py: Python<'_> = connection.py();
        match Self::metadata_value(connection, SQLITE_NAMESPACE_KEY)? {
            Some(value) => Ok(value.str()?.extract()?),
            None => Err(storage_error(
                py,
                "SQLite history cursor namespace is missing",
            )),
        }
    }

    pub(crate) fn stored_event<'py>(
        row: &Bound<'py, PyAny>,
        namespace: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
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

    pub(crate) fn read_all_runs(connection: &Bound<'_, PyAny>) -> PyResult<Vec<RunRow>> {
        execute_plain(connection, SQLITE_ALL_RUNS)?
            .try_iter()?
            .map(|row| {
                row.and_then(|row| crate::execution_history::_helpers::sqlite::stored_run(&row))
            })
            .collect()
    }

    pub(crate) fn replace_projection(
        connection: &Bound<'_, PyAny>,
        projected: &[RunRow],
    ) -> PyResult<()> {
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

    pub(crate) fn append_prepared<'py>(
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
                    let digest: Option<String> =
                        optional_text(&column(&existing, "content_digest")?)?;
                    let payload: Option<String> =
                        optional_text(&column(&existing, "payload_json")?)?;
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

    pub(crate) fn append_events_in<'py>(
        connection: &Bound<'py, PyAny>,
        events: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let prepared: Vec<PreparedEvent> = prepared_batch(events)?;
        Self::append_prepared(connection, &prepared)
    }

    pub(crate) fn migrate_in(&self, slf: &Bound<'_, Self>) -> PyResult<()> {
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

    pub(crate) fn run_cursor_key(
        connection: &Bound<'_, PyAny>,
        cursor: &Bound<'_, PyAny>,
    ) -> PyResult<Option<RunCursorKey>> {
        let py: Python<'_> = connection.py();
        if cursor.is_none() {
            return Ok(None);
        }
        let text: String = optional_text(cursor)?.unwrap_or_default();
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
    pub(crate) fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        Self {
            path: OnceLock::new(),
            lock: ReentrantLock::default(),
            closed: AtomicBool::new(true),
            project_calls: AtomicI64::new(0),
        }
    }

    #[pyo3(signature = (project_dir = None, *, path = None, busy_timeout_ms = None))]
    pub(crate) fn __init__(
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
        let is_storage: bool = history_error_of_kind(
            py,
            &error,
            crate::execution_history::_helpers::sqlite::STORAGE_ERROR,
        )?;
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
    pub fn path<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let path: &str = self.database_path();
        if path == MEMORY_PATH {
            return Ok(py.None().into_bound(py));
        }
        imported(py, PATHLIB_MODULE, "Path")?.call1((path,))
    }

    /// The number of incremental projection calls.
    #[getter]
    pub fn project_calls(&self) -> i64 {
        self.project_calls.load(Ordering::SeqCst)
    }

    pub(crate) fn append_event<'py>(
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

    pub(crate) fn append_events<'py>(
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
    pub(crate) fn stored_events_page<'py>(
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
            sqlbuild_python_base::boundary::main::call_with::call_with(
                &host_entry(py, ExecutorHost::EventPage)?,
                &[
                    ("records", tuple_of(py, records)?.into_any()),
                    ("next_cursor", next_cursor),
                    ("has_more", has_more.into_bound_py_any(py)?),
                ],
            )
        })
    }

    #[pyo3(name = "get_run")]
    pub(crate) fn run_record<'py>(
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
                Some(row) => crate::execution_history::_helpers::conversion::run_record(
                    py,
                    &crate::execution_history::_helpers::sqlite::stored_run(&row)?,
                ),
                None => Ok(py.None().into_bound(py)),
            }
        })
    }

    #[pyo3(signature = (*, run_filter, after_cursor = None, limit = None))]
    #[pyo3(name = "get_runs")]
    pub(crate) fn run_page<'py>(
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
                .map(crate::execution_history::_helpers::sqlite::stored_run)
                .collect::<PyResult<_>>()?;
            let next_cursor: Bound<'py, PyAny> = match runs.last() {
                Some(run) => {
                    let namespace: String = Self::namespace(&connection)?;
                    checked(py, run_cursor(Dialect::Sqlite, &namespace, run))?
                        .into_bound_py_any(py)?
                }
                None => py.None().into_bound(py),
            };
            sqlbuild_python_base::boundary::main::call_with::call_with(
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
        })
    }

    pub(crate) fn project<'py>(
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

    pub(crate) fn rebuild_from_events<'py>(
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
    pub(crate) fn append_and_project<'py>(
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
    pub(crate) fn reconcile<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
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
    pub(crate) fn check_health(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            let connection: Bound<'_, PyAny> = native_field(slf, "_connection")?;
            let Some(result) = fetch_one(&execute_plain(&connection, SQLITE_QUICK_CHECK)?)? else {
                return Ok(false);
            };
            if optional_text(&result.get_item(0)?)?.as_deref() != Some(SQLITE_HEALTHY_CHECK_RESULT)
            {
                return Ok(false);
            }
            let version: i64 = slf.call_method0("get_schema_version")?.extract()?;
            Ok(version == SCHEMA_VERSION)
        })
    }

    #[pyo3(name = "get_schema_version")]
    pub(crate) fn schema_version(slf: &Bound<'_, Self>) -> PyResult<i64> {
        let py: Python<'_> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || {
            this.ensure_open(py)?;
            Self::metadata_version(&native_field(slf, "_connection")?)
        })
    }

    #[pyo3(signature = (*, target_version = None))]
    pub(crate) fn upgrade_schema(
        slf: &Bound<'_, Self>,
        target_version: Option<i64>,
    ) -> PyResult<i64> {
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

    pub(crate) fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
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

    pub(crate) fn dispose(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.call_method0("close")?;
        Ok(())
    }

    pub(crate) fn __enter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        let py: Python<'py> = slf.py();
        let this: &Self = slf.get();
        this.with_lock(py, || this.ensure_open(py))?;
        Ok(slf.clone())
    }

    #[pyo3(signature = (*_exc_info))]
    pub(crate) fn __exit__(
        slf: &Bound<'_, Self>,
        _exc_info: &Bound<'_, PyTuple>,
    ) -> PyResult<Option<bool>> {
        slf.call_method0("close")?;
        Ok(None)
    }

    pub(crate) fn _migrate(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.get().migrate_in(slf)
    }

    pub(crate) fn _read_all_events<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
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

    pub(crate) fn _publish_projection(
        slf: &Bound<'_, Self>,
        projected: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
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
    pub(crate) fn without_conflicts<T>(
        py: Python<'_>,
        result: PyResult<T>,
        message: &str,
    ) -> PyResult<T> {
        match result {
            Err(error) if history_error_of_kind(py, &error, INTEGRITY_CONFLICT)? => Err(error),
            result => wrapped(py, result, message),
        }
    }

    pub(crate) fn event_position(
        connection: &Bound<'_, PyAny>,
        cursor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<i64> {
        let py: Python<'_> = connection.py();
        let Some(cursor) = cursor.filter(|cursor| !cursor.is_none()) else {
            return Ok(0);
        };
        let text: String = optional_text(cursor)?.unwrap_or_default();
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

//! Rust-owned SQLite connection; Python views exist only for connection overrides.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyclass, pymethods};
use rusqlite::types::Value;
use rusqlite::{Connection, Error, ErrorCode, OpenFlags, params_from_iter};
use sqlbuild_state::execution_history::constants::MEMORY_PATH;
use sqlbuild_state::execution_history::models::SqlParam;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::native_host::sqlite_cursor::{SqliteCursor, SqliteRow};

/// Preserve DB-API exception classes at the public storage-error boundary.
fn database_error(py: Python<'_>, error: Error) -> PyErr {
    let class: &str = match &error {
        Error::SqliteFailure(error, _) if error.code == ErrorCode::ConstraintViolation => {
            "IntegrityError"
        }
        Error::SqliteFailure(error, _)
            if matches!(
                error.code,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
            ) =>
        {
            "DatabaseError"
        }
        _ => "OperationalError",
    };
    match imported(py, "sqlite3", class).and_then(|class| class.call1((error.to_string(),))) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// Parameters of history SQL contain no Python-owned values.
pub(crate) fn sqlite_params(params: &[SqlParam]) -> PyResult<Vec<Value>> {
    params
        .iter()
        .map(|value| {
            Ok(match value {
                SqlParam::Null => Value::Null,
                SqlParam::Text(value) => Value::Text(value.clone()),
                SqlParam::Integer(value) => Value::Integer(*value),
                SqlParam::Flag(value) => Value::Integer(i64::from(*value)),
                SqlParam::Timestamp(_) | SqlParam::TextList(_) => {
                    return Err(PyValueError::new_err("Postgres-only history parameter"));
                }
            })
        })
        .collect()
}

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteConnection")]
pub(crate) struct SqliteConnection {
    connection: Mutex<Option<Connection>>,
}

impl SqliteConnection {
    /// Open literal filenames, using absolute paths because bundled SQLite enables URI handling globally.
    pub(crate) fn open(py: Python<'_>, path: &str) -> PyResult<Self> {
        let flags = OpenFlags::default() & !OpenFlags::SQLITE_OPEN_URI;
        let database: PathBuf = if path == MEMORY_PATH {
            PathBuf::from(path)
        } else {
            std::env::current_dir()?.join(path)
        };
        let connection = py
            .detach(|| Connection::open_with_flags(database, flags))
            .map_err(|error| database_error(py, error))?;
        Ok(Self {
            connection: Mutex::new(Some(connection)),
        })
    }

    fn with_connection<T: Send>(
        &self,
        py: Python<'_>,
        body: impl FnOnce(&Connection) -> rusqlite::Result<T> + Send,
    ) -> PyResult<T> {
        let result = py.detach(|| {
            let guard = self
                .connection
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            guard.as_ref().map(body)
        });
        match result {
            Some(result) => result.map_err(|error| database_error(py, error)),
            None => Err(PyErr::from_value(
                imported(py, "sqlite3", "ProgrammingError")?
                    .call1(("Cannot operate on a closed database.",))?,
            )),
        }
    }

    pub(crate) fn execute_native(
        &self,
        py: Python<'_>,
        sql: &str,
        params: &[Value],
    ) -> PyResult<SqliteCursor> {
        self.with_connection(py, |connection| {
            let mut statement = connection.prepare_cached(sql)?;
            let columns = Arc::new(
                statement
                    .column_names()
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            );
            let count: usize = statement.column_count();
            let mut cursor = statement.query(params_from_iter(params))?;
            let mut rows: VecDeque<SqliteRow> = VecDeque::new();
            while let Some(row) = cursor.next()? {
                rows.push_back(SqliteRow {
                    columns: Arc::clone(&columns),
                    values: (0..count)
                        .map(|index| row.get(index))
                        .collect::<rusqlite::Result<_>>()?,
                });
            }
            Ok(SqliteCursor {
                rows: Mutex::new(rows),
                last_row_id: connection.last_insert_rowid(),
            })
        })
    }
}

#[pymethods]
impl SqliteConnection {
    #[pyo3(signature = (sql, parameters = None))]
    fn execute(
        &self,
        py: Python<'_>,
        sql: &str,
        parameters: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<SqliteCursor> {
        let mut params: Vec<Value> = Vec::new();
        if let Some(parameters) = parameters {
            for value in parameters.try_iter()? {
                let value = value?;
                params.push(if value.is_none() {
                    Value::Null
                } else if let Ok(value) = value.extract::<i64>() {
                    Value::Integer(value)
                } else {
                    Value::Text(value.extract()?)
                });
            }
        }
        self.execute_native(py, sql, &params)
    }

    fn executemany(
        &self,
        py: Python<'_>,
        sql: &str,
        parameters: &Bound<'_, PyAny>,
    ) -> PyResult<SqliteCursor> {
        let mut result = SqliteCursor {
            rows: Mutex::default(),
            last_row_id: 0,
        };
        for row in parameters.try_iter()? {
            result = self.execute(py, sql, Some(&row?))?;
        }
        Ok(result)
    }

    #[getter]
    pub(crate) fn in_transaction(&self, py: Python<'_>) -> PyResult<bool> {
        self.with_connection(py, |connection| Ok(!connection.is_autocommit()))
    }

    pub(crate) fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            self.connection
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
        });
        Ok(())
    }
}

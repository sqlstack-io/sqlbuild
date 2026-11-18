//! Public records of warehouse access.

use crate::boundary::main::imported::imported;
use crate::warehouse_access::_helpers::sqlite_connection::database_error;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::types::PyBytes;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::params_from_iter;
use rusqlite::types::Value;
use sqlbuild_state::execution_history::constants::MEMORY_PATH;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteConnection")]
#[derive(Debug)]
pub struct SqliteConnection {
    pub(crate) connection: Mutex<Option<Connection>>,
}

impl SqliteConnection {
    /// Open literal filenames, using absolute paths because bundled SQLite enables URI handling globally.
    pub fn open(py: Python<'_>, path: &str) -> PyResult<Self> {
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

    pub(crate) fn with_connection<T: Send>(
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
            None => Err(pyo3::prelude::PyErr::from_value(
                imported(py, "sqlite3", "ProgrammingError")?
                    .call1(("Cannot operate on a closed database.",))?,
            )),
        }
    }

    pub fn execute_native(
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
    pub(crate) fn execute(
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

    pub(crate) fn executemany(
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
    pub fn in_transaction(&self, py: Python<'_>) -> PyResult<bool> {
        self.with_connection(py, |connection| Ok(!connection.is_autocommit()))
    }

    pub fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            self.connection
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
        });
        Ok(())
    }
}

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteRow")]
#[derive(Debug)]
pub struct SqliteRow {
    pub columns: Arc<Vec<String>>,
    pub values: Vec<Value>,
}

impl SqliteRow {
    pub fn value<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyAny>> {
        match self.values.get(index) {
            Some(Value::Null) => Ok(py.None().into_bound(py)),
            Some(Value::Integer(value)) => value.into_bound_py_any(py),
            Some(Value::Real(value)) => value.into_bound_py_any(py),
            Some(Value::Text(value)) => value.into_bound_py_any(py),
            Some(Value::Blob(value)) => Ok(PyBytes::new(py, value).into_any()),
            None => Err(PyIndexError::new_err("tuple index out of range")),
        }
    }

    pub fn column<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let index: usize = self
            .columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case(name))
            .ok_or_else(|| PyIndexError::new_err("No item with that key"))?;
        self.value(py, index)
    }
}

#[pymethods]
impl SqliteRow {
    pub(crate) fn __getitem__<'py>(&self, key: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        if let Ok(index) = key.extract::<isize>() {
            let index = if index < 0 {
                self.values.len().checked_add_signed(index)
            } else {
                Some(index as usize)
            };
            return self.value(key.py(), index.unwrap_or(usize::MAX));
        }
        if let Ok(name) = key.extract::<String>() {
            return self.column(key.py(), &name);
        }
        Err(PyTypeError::new_err("Index must be int or string"))
    }
}

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteCursor")]
#[derive(Debug)]
pub struct SqliteCursor {
    pub rows: Mutex<VecDeque<SqliteRow>>,
    pub last_row_id: i64,
}

impl SqliteCursor {
    pub fn next_row(&self) -> Option<SqliteRow> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
    }
}

#[pymethods]
impl SqliteCursor {
    pub(crate) fn __iter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    pub(crate) fn __next__(&self) -> Option<SqliteRow> {
        self.next_row()
    }

    pub(crate) fn fetchone(&self) -> Option<SqliteRow> {
        self.next_row()
    }

    pub(crate) fn fetchall(&self) -> Vec<SqliteRow> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    #[getter]
    pub(crate) fn lastrowid(&self) -> i64 {
        self.last_row_id
    }
}

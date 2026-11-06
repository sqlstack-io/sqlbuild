//! Native SQLite rows, with a DB-API view for the existing connection override boundary.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyclass, pymethods};
use pyo3::types::PyBytes;
use rusqlite::types::Value;

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteRow")]
pub(crate) struct SqliteRow {
    pub(crate) columns: Arc<Vec<String>>,
    pub(crate) values: Vec<Value>,
}

impl SqliteRow {
    pub(crate) fn value<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyAny>> {
        match self.values.get(index) {
            Some(Value::Null) => Ok(py.None().into_bound(py)),
            Some(Value::Integer(value)) => value.into_bound_py_any(py),
            Some(Value::Real(value)) => value.into_bound_py_any(py),
            Some(Value::Text(value)) => value.into_bound_py_any(py),
            Some(Value::Blob(value)) => Ok(PyBytes::new(py, value).into_any()),
            None => Err(PyIndexError::new_err("tuple index out of range")),
        }
    }

    pub(crate) fn column<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
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
    fn __getitem__<'py>(&self, key: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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
pub(crate) struct SqliteCursor {
    pub(crate) rows: Mutex<VecDeque<SqliteRow>>,
    pub(crate) last_row_id: i64,
}

impl SqliteCursor {
    pub(crate) fn next_row(&self) -> Option<SqliteRow> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
    }
}

#[pymethods]
impl SqliteCursor {
    fn __iter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    fn __next__(&self) -> Option<SqliteRow> {
        self.next_row()
    }

    fn fetchone(&self) -> Option<SqliteRow> {
        self.next_row()
    }

    fn fetchall(&self) -> Vec<SqliteRow> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    #[getter]
    fn lastrowid(&self) -> i64 {
        self.last_row_id
    }
}

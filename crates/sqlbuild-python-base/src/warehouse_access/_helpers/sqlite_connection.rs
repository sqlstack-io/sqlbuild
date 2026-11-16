//! Rust-owned SQLite connection and its rows; Python views exist only for connection overrides.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods, PyErr, PyResult, Python};
use rusqlite::types::Value;
use rusqlite::{Error, ErrorCode};
use sqlbuild_state::execution_history::models::SqlParam;

use crate::boundary::main::imported::imported;

/// Preserve DB-API exception classes at the public storage-error boundary.
pub(crate) fn database_error(py: Python<'_>, error: Error) -> PyErr {
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

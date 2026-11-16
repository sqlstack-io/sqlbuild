//! Parameters of history SQL contain no Python-owned values.

use crate::warehouse_access::_helpers::sqlite_connection;
use pyo3::prelude::PyResult;
use rusqlite::types::Value;
use sqlbuild_state::execution_history::models::SqlParam;

/// Parameters of history SQL contain no Python-owned values.
#[inline]
pub fn sqlite_params(params: &[SqlParam]) -> PyResult<Vec<Value>> {
    sqlite_connection::sqlite_params(params)
}

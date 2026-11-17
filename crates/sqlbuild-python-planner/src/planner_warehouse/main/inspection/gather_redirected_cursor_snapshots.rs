//! Cursor snapshots of models whose history lives in another relation or column.

use crate::planner_warehouse::_helpers::inspection::snapshot;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict as KeywordDict;

/// Cursor snapshots of models whose history lives in another relation or column.
#[inline]
pub fn gather_redirected_cursor_snapshots<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    snapshot::gather_redirected_cursor_snapshots(py, options)
}

//! Gather source columns.

use crate::planner_warehouse::_helpers::sources::source_columns;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Gather source columns.
#[inline]
pub fn gather_source_columns<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    source_columns::gather_source_columns(project, adapter, connection, source_entries)
}

//! Gather source inspection.

use crate::planner_warehouse::_helpers::sources::source_columns;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Gather source inspection.
#[inline]
pub fn gather_source_inspection<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    source_columns::gather_source_inspection(project, adapter, connection, source_entries)
}

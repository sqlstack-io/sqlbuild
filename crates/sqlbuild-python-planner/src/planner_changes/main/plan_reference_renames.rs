//! This plan's renames plus recorded renames, read only when a selected query that references models changed.

use crate::planner_changes::_helpers::renames;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// This plan's renames plus recorded renames, read only when a selected query that references models changed.
#[inline]
pub fn plan_reference_renames<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    renames::plan_reference_renames(py, options)
}

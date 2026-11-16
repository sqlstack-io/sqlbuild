//! Warn for selected models stale through changed upstreams outside the run set.

use crate::planner_identity::_helpers::staleness;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// Warn for selected models stale through changed upstreams outside the run set.
#[inline]
pub fn build_stale_out_of_selection_warnings<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    staleness::build_stale_out_of_selection_warnings(py, options)
}

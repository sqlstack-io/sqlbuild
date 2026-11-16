//! Model, source, seed and function selectors resolved against compiled graph mappings.

use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyFrozenSet;

/// Model, source, seed and function selectors resolved against compiled graph mappings.
#[inline]
pub fn resolve_project_selectors<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    selectors::resolve_project_selectors(py, options)
}

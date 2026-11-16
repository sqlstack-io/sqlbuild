//! The keys whitespace-separated selector tokens match, without required build resources.

use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyFrozenSet;

/// The keys whitespace-separated selector tokens match, without required build resources.
#[inline]
pub fn resolve_project_selector_tokens<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    selectors::resolve_project_selector_tokens(py, options)
}

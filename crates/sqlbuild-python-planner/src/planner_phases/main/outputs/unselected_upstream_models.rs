//! The unselected models the selected scope reads, sorted.

use crate::planner_phases::_helpers::outputs::output_assembly;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The unselected models the selected scope reads, sorted.
#[inline]
pub fn unselected_upstream_models(scopes: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    output_assembly::unselected_upstream_models(scopes)
}

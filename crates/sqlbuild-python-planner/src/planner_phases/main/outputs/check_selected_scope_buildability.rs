//! Check selected scope buildability.

use crate::planner_phases::_helpers::outputs::buildability;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Check selected scope buildability.
#[inline]
pub fn check_selected_scope_buildability<'py>(
    runtime: &Bound<'py, PyAny>,
    scopes: &Bound<'py, PyAny>,
    warehouse: &Bound<'py, PyAny>,
    deferral: &Bound<'py, PyAny>,
) -> PyResult<()> {
    buildability::check_selected_scope_buildability(runtime, scopes, warehouse, deferral)
}

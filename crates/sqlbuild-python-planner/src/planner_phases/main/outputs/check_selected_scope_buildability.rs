//! Check selected scope buildability.

use crate::planner_phases::_helpers::outputs::buildability;
use pyo3::prelude::{Bound, PyAny, PyResult};
use std::collections::BTreeMap;

/// Check selected scope buildability; returns the end-of-build audits that won't run, by name with the reads they miss.
#[inline]
pub fn check_selected_scope_buildability<'py>(
    runtime: &Bound<'py, PyAny>,
    scopes: &Bound<'py, PyAny>,
    warehouse: &Bound<'py, PyAny>,
    deferral: &Bound<'py, PyAny>,
) -> PyResult<BTreeMap<String, Vec<String>>> {
    buildability::check_selected_scope_buildability(runtime, scopes, warehouse, deferral)
}

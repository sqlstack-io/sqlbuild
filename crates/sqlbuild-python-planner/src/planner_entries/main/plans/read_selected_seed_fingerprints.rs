//! Read only fingerprint state required by selected direct seeds.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// Read only fingerprint state required by selected direct seeds.
#[inline]
pub fn read_selected_seed_fingerprints<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    seeds: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    command_plans::read_selected_seed_fingerprints(adapter, connection, seeds)
}

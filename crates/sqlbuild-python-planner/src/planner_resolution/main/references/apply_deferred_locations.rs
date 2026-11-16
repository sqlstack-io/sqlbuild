//! Apply deferred locations.

use crate::planner_resolution::_helpers::references::refs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Apply deferred locations.
#[inline]
pub fn apply_deferred_locations<'py>(
    model_locations: &Bound<'py, PyDict>,
    seed_locations: &Bound<'py, PyDict>,
    deferred_locations: &Bound<'py, PyDict>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
    refs::apply_deferred_locations(
        model_locations,
        seed_locations,
        deferred_locations,
        selected_keys,
    )
}

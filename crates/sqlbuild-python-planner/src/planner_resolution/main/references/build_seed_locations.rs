//! Build seed locations.

use crate::planner_resolution::_helpers::references::locations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Build seed locations.
#[inline]
pub fn build_seed_locations<'py>(seeds: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    locations::build_seed_locations(seeds)
}

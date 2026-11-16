//! Build seed locations.

use crate::planner_resolution::_helpers::references::locations::locations;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict;

#[pyfunction]
pub fn build_seed_locations<'py>(seeds: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    locations(seeds)
}

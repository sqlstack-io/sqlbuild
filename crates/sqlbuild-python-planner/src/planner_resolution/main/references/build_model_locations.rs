//! Build model locations.

use crate::planner_resolution::_helpers::references::locations::locations;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict;

#[pyfunction]
pub fn build_model_locations<'py>(models: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    locations(models)
}

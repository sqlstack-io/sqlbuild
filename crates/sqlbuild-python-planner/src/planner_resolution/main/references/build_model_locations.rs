//! Build model locations.

use crate::planner_resolution::_helpers::references::locations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Build model locations.
#[inline]
pub fn build_model_locations<'py>(models: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    locations::build_model_locations(models)
}

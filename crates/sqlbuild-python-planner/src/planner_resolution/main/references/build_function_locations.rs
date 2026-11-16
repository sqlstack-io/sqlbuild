//! Build function locations.

use crate::planner_resolution::_helpers::references::locations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Build function locations.
#[inline]
pub fn build_function_locations<'py>(
    functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    locations::build_function_locations(functions)
}

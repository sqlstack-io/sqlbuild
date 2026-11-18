//! The SQL dialect of a planner runtime passed as a Python object.

use crate::planner_phases::_helpers::inputs::runtime;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::native_host::models::Dialect;

/// The SQL dialect of a planner runtime passed as a Python object.
#[inline]
pub fn runtime_dialect<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Bound<'py, Dialect>> {
    runtime::runtime_dialect(runtime)
}

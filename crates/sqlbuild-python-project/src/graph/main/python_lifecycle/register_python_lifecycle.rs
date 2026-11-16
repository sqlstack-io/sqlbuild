//! Register the python lifecycle functions and classes.

use crate::graph::_helpers::python_lifecycle;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the python lifecycle functions and classes.
#[inline]
pub fn register_python_lifecycle(module: &Bound<'_, PyModule>) -> PyResult<()> {
    python_lifecycle::register(module)
}

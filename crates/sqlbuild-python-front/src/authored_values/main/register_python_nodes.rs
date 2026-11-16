//! Register the python nodes functions and classes.

use crate::authored_values::_helpers::python_nodes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the python nodes functions and classes.
#[inline]
pub fn register_python_nodes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    python_nodes::register(module)
}

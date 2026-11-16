//! Register the python selection functions and classes.

use crate::graph::_helpers::python_selection;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the python selection functions and classes.
#[inline]
pub fn register_python_selection(module: &Bound<'_, PyModule>) -> PyResult<()> {
    python_selection::register(module)
}

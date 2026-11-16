//! Register the imported python nodes functions and classes.

use crate::discovery::_helpers::python_files::imported_python_nodes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the imported python nodes functions and classes.
#[inline]
pub fn register_imported_python_nodes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    imported_python_nodes::register(module)
}

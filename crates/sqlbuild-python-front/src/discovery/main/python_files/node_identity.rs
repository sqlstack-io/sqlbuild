//! The identity of one discovered node of `(kind, documented)`, hashed once by the user-code host.

use crate::discovery::_helpers::python_files::imported_python_nodes;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// The identity of one discovered node of `(kind, documented)`, hashed once by the user-code host.
#[inline]
pub fn node_identity<'py>(
    py: Python<'py>,
    (kind, documented): (&str, bool),
    get: &dyn Fn(&str) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    imported_python_nodes::node_identity(py, (kind, documented), get)
}

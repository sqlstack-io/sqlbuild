//! Any Rust value as a Python object.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// Any Rust value as a Python object.
pub fn any<'py, T: IntoPyObjectExt<'py>>(py: Python<'py>, value: T) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

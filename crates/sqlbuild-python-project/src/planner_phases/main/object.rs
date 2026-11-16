//! A native helper's Rust result as a Python object.

use pyo3::prelude::{Bound, PyAny, PyErr, PyResult, Python};
use pyo3::{BoundObject, IntoPyObject};

/// A native helper's Rust result as a Python object.
pub fn object<'py, T>(py: Python<'py>, value: T) -> PyResult<Bound<'py, PyAny>>
where
    T: IntoPyObject<'py>,
    PyErr: From<T::Error>,
{
    Ok(value.into_pyobject(py)?.into_bound().into_any())
}

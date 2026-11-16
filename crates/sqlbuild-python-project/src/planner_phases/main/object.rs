//! A native helper's Rust result as a Python object.

use crate::planner_phases::_helpers::objects;
use pyo3::IntoPyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// A native helper's Rust result as a Python object.
#[inline]
pub fn object<'py, T>(py: Python<'py>, value: T) -> PyResult<Bound<'py, PyAny>>
where
    T: IntoPyObject<'py>,
    PyErr: From<T::Error>,
{
    objects::object(py, value)
}

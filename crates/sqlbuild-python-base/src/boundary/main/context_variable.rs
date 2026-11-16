//! The `contextvars.ContextVar(name, default=None)` kept in `cell`, created once per interpreter.

use crate::boundary::_helpers::python_calls;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::sync::PyOnceLock;

/// The `contextvars.ContextVar(name, default=None)` kept in `cell`, created once per interpreter.
#[inline]
pub fn context_variable<'py>(
    py: Python<'py>,
    cell: &'static PyOnceLock<Py<PyAny>>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    python_calls::context_variable(py, cell, name)
}

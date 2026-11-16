//! `module.name`, imported.

use crate::boundary::_helpers::python_calls;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `module.name`, imported.
#[inline]
pub fn imported<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    python_calls::imported(py, module, name)
}

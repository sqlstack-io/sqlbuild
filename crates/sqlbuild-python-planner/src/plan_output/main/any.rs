//! Any Rust value as a Python object.

use crate::plan_output::_helpers::values;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Any Rust value as a Python object.
#[inline]
pub fn any<'py, T: IntoPyObjectExt<'py>>(py: Python<'py>, value: T) -> PyResult<Bound<'py, PyAny>> {
    values::any(py, value)
}

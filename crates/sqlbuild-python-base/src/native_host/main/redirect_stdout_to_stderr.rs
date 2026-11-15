//! Keep user prints off machine-readable stdout while running project code.

use crate::native_host::_helpers::host;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Keep user prints off machine-readable stdout while running project code.
#[inline]
pub fn redirect_stdout_to_stderr(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    host::redirect_stdout_to_stderr(py)
}

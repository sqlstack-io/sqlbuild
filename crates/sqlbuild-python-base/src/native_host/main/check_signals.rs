//! Check both pending Python signals and a Ctrl+C whose exception a Python callback swallowed.

use crate::native_host::_helpers::interrupt;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Check both pending Python signals and a Ctrl+C whose exception a Python callback swallowed.
#[inline]
pub fn check_signals(py: Python<'_>) -> PyResult<()> {
    interrupt::check_signals(py)
}

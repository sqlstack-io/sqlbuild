//! `logging.getLogger(logger).warning(message)`.

use crate::native_host::_helpers::host;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `logging.getLogger(logger).warning(message)`.
#[inline]
pub fn log_warning_message(py: Python<'_>, logger: &str, message: &str) -> PyResult<()> {
    host::log_warning_message(py, logger, message)
}

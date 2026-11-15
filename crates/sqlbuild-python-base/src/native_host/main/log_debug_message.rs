//! `log_debug_event(logger=logging.getLogger(logger), message=message)`.

use crate::native_host::_helpers::host;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `log_debug_event(logger=logging.getLogger(logger), message=message)`.
#[inline]
pub fn log_debug_message(py: Python<'_>, logger: &str, message: &str) -> PyResult<()> {
    host::log_debug_message(py, logger, message)
}

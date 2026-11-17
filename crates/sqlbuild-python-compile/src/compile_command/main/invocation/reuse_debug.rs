//! Log one message at debug level on the compile reuse logger.

use crate::compile_command::_helpers::invocation::reuse_attempt::LOG_TEXT;
use crate::compile_command::_helpers::invocation::reuse_attempt::reuse_debug_with;
use pyo3::prelude::{PyResult, Python};

/// Log one message at debug level on the compile reuse logger.
pub fn reuse_debug(py: Python<'_>, message: &str) -> PyResult<()> {
    reuse_debug_with(py, LOG_TEXT, message)
}

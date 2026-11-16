//! Note a CTX lookup; only the per-invocation run identity is volatile.

use crate::compile_context::_helpers::input_reads;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Note a CTX lookup; only the per-invocation run identity is volatile.
#[inline]
pub fn context_read(py: Python<'_>, name: &str) -> PyResult<()> {
    input_reads::context_read(py, name)
}

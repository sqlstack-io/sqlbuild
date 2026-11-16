//! Note that a template looked up one environment variable, present or not.

use crate::compile_context::_helpers::input_reads;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Note that a template looked up one environment variable, present or not.
#[inline]
pub fn environment_read(py: Python<'_>, name: &str) -> PyResult<()> {
    input_reads::environment_read(py, name)
}

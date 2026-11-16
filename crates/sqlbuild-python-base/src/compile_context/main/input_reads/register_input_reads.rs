//! Register the input reads functions and classes.

use crate::compile_context::_helpers::input_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the input reads functions and classes.
#[inline]
pub fn register_input_reads(module: &Bound<'_, PyModule>) -> PyResult<()> {
    input_reads::register(module)
}

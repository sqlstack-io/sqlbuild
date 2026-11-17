//! Register the text functions and classes.

use crate::output_capture::_helpers::text::chunk_command_output_text;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};

/// Register the text functions and classes.
pub fn register_text(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(chunk_command_output_text, module)?)
}

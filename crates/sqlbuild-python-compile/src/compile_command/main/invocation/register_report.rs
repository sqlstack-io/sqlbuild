//! Register the report functions and classes.

use crate::compile_command::_helpers::invocation::report::format_compile_json;
use crate::compile_command::_helpers::invocation::report::format_compile_text;
use crate::compile_command::main::invocation::format_compile_error_json::format_compile_error_json;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the report functions and classes.
pub fn register_report(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_compile_text, module)?)?;
    module.add_function(wrap_pyfunction!(format_compile_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_compile_error_json, module)?)
}

//! Register the records functions and classes.

use crate::output_capture::_helpers::records::command_output_from_json;
use crate::output_capture::_helpers::records::command_output_to_json;
use crate::output_capture::_helpers::records::freeze_command_output_record;
use crate::output_capture::_helpers::records::invocation_context_from_environment;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};

/// Register the records functions and classes.
pub fn register_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(freeze_command_output_record, module)?)?;
    module.add_function(wrap_pyfunction!(command_output_to_json, module)?)?;
    module.add_function(wrap_pyfunction!(command_output_from_json, module)?)?;
    module.add_function(wrap_pyfunction!(
        invocation_context_from_environment,
        module
    )?)
}

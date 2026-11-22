//! Register the context functions and classes.

use crate::output_capture::_helpers::context::CommandOutputContext;
use crate::output_capture::_helpers::context::OutputNodeScope;
use crate::output_capture::_helpers::context::command_output_context;
use crate::output_capture::_helpers::context::current_output_capture_context;
use crate::output_capture::_helpers::context::output_node_scope;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};

/// Register the context functions and classes.
pub fn register_context(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CommandOutputContext>()?;
    module.add_class::<OutputNodeScope>()?;
    module.add_function(wrap_pyfunction!(output_node_scope, module)?)?;
    module.add_function(wrap_pyfunction!(command_output_context, module)?)?;
    module.add_function(wrap_pyfunction!(current_output_capture_context, module)?)
}

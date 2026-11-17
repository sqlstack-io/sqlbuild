//! Register the attachment functions and classes.

use crate::input_loops::_helpers::resources::attachment::attach_resources;
use crate::input_loops::main::attach_checks::attach_checks;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the attachment functions and classes.
pub fn register_attachment(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(attach_resources, module)?)?;
    module.add_function(wrap_pyfunction!(attach_checks, module)?)
}

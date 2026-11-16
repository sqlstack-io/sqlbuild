//! Register the syntax checks functions and classes.

use crate::project_assembly::_helpers::syntax_checks::standalone_pivot_models;
use crate::project_assembly::main::model_placeholders::model_placeholders;
use crate::project_assembly::main::model_syntax_checks::model_syntax_checks;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the syntax checks functions and classes.
pub fn register_syntax_checks(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_placeholders, module)?)?;
    module.add_function(wrap_pyfunction!(model_syntax_checks, module)?)?;
    module.add_function(wrap_pyfunction!(standalone_pivot_models, module)?)?;
    Ok(())
}

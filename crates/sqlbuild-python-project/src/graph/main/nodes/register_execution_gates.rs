//! Register the execution gates functions and classes.

use crate::graph::main::nodes::model_python_hook_names::model_python_hook_names;
use crate::graph::main::nodes::sql_ref_key::sql_ref_key;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the execution gates functions and classes.
pub fn register_execution_gates(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_python_hook_names, module)?)?;
    module.add_function(wrap_pyfunction!(sql_ref_key, module)?)?;
    Ok(())
}

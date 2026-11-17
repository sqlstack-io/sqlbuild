//! Register the relation targets functions and classes.

use crate::compile_driver::main::setup::build_project_relation_targets::build_project_relation_targets;
use crate::compile_driver::main::setup::build_python_relation_targets::build_python_relation_targets;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the relation targets functions and classes.
pub fn register_relation_targets(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_python_relation_targets, module)?)?;
    module.add_function(wrap_pyfunction!(build_project_relation_targets, module)?)
}

//! Register the stage functions and classes.

use crate::semantic_checks::_helpers::session::stage::complete_semantic_project;
use crate::semantic_checks::_helpers::session::stage::semantic_metadata_diagnostics;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the stage functions and classes.
pub fn register_stage(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(complete_semantic_project, module)?)?;
    module.add_function(wrap_pyfunction!(semantic_metadata_diagnostics, module)?)
}

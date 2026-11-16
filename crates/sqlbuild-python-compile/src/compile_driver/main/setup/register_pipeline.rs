//! Register the pipeline functions and classes.

use crate::compile_driver::main::setup::compile_project_phase::compile_project_phase;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the pipeline functions and classes.
pub fn register_pipeline(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_project_phase, module)?)?;
    Ok(())
}

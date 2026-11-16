//! Register the analysis functions and classes.

use crate::compile_driver::main::stages::analyze_compile_project::analyze_compile_project;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the analysis functions and classes.
pub fn register_analysis(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(analyze_compile_project, module)?)
}

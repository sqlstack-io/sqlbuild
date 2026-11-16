//! Register the project functions and classes.

use crate::compile_driver::_helpers::setup::project::build_project_graph_with_analysis_selection;
use crate::compile_driver::main::setup::build_compiled_project::build_compiled_project;
use crate::compile_driver::main::setup::build_project_graph::build_project_graph;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the project functions and classes.
pub fn register_project(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_compiled_project, module)?)?;
    module.add_function(wrap_pyfunction!(
        build_project_graph_with_analysis_selection,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_project_graph, module)?)
}

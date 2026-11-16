//! Register the pipeline entries functions and classes.

use crate::compile_driver::_helpers::setup::pipeline_entries::resolve_compile_analysis_selection;
use crate::compile_driver::main::setup::build_project_graph_from_compiled_project::build_project_graph_from_compiled_project;
use crate::compile_driver::main::setup::build_static_pipeline_result::build_static_pipeline_result;
use crate::compile_driver::main::setup::plan_has_executable_work::plan_has_executable_work;
use crate::compile_driver::main::setup::run_compile_only_pipeline::run_compile_only_pipeline;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the pipeline entries functions and classes.
pub fn register_pipeline_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        resolve_compile_analysis_selection,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_project_graph_from_compiled_project,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(plan_has_executable_work, module)?)?;
    module.add_function(wrap_pyfunction!(build_static_pipeline_result, module)?)?;
    module.add_function(wrap_pyfunction!(run_compile_only_pipeline, module)?)
}

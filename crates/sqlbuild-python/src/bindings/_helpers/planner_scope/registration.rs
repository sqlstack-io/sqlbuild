//! Register planner scope and loader entry points.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use sqlbuild_python_project::planner_scope::main::build_intermediate_source_map_function::build_intermediate_source_map_function;
use sqlbuild_python_project::planner_scope::main::build_source_load_entries_function::build_source_load_entries_function;
use sqlbuild_python_project::planner_scope::main::build_source_load_map_function::build_source_load_map_function;
use sqlbuild_python_project::planner_scope::main::expand_selected_loader_dependencies_function::expand_selected_loader_dependencies_function;
use sqlbuild_python_project::planner_scope::main::upstream_loader_dependency_names_function::upstream_loader_dependency_names_function;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(upstream_loader_dependency_names_function(module)?)?;
    module.add_function(build_intermediate_source_map_function(module)?)?;
    module.add_function(expand_selected_loader_dependencies_function(module)?)?;
    module.add_function(build_source_load_map_function(module)?)?;
    module.add_function(build_source_load_entries_function(module)?)
}

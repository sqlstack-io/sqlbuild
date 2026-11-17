//! Register planner scope and loader entry points.

use crate::bindings::_helpers::planner_scope::loaders;
use crate::bindings::_helpers::planner_scope::sources;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        loaders::upstream_loader_dependency_names,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        loaders::build_intermediate_source_map,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        loaders::expand_selected_loader_dependencies,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(sources::build_source_load_map, module)?)?;
    module.add_function(wrap_pyfunction!(
        sources::build_source_load_entries,
        module
    )?)
}

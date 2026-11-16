//! `build_intermediate_source_map` as a function of `module`.

use crate::planner_scope::_helpers::loaders;

/// `build_intermediate_source_map` as a function of `module`.
#[inline]
pub fn build_intermediate_source_map_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    loaders::build_intermediate_source_map_function(module)
}

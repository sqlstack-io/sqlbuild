//! `build_intermediate_source_map` as a function of `module`.

use crate::planner_scope::main::build_intermediate_source_map::build_intermediate_source_map;

/// `build_intermediate_source_map` as a function of `module`.
pub fn build_intermediate_source_map_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_intermediate_source_map, module)
}

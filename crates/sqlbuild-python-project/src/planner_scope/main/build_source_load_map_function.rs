//! `build_source_load_map` as a function of `module`.

use crate::planner_scope::_helpers::sources::build_source_load_map;

/// `build_source_load_map` as a function of `module`.
pub fn build_source_load_map_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_source_load_map, module)
}

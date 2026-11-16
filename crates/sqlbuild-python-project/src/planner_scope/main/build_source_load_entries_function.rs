//! `build_source_load_entries` as a function of `module`.

use crate::planner_scope::_helpers::sources::build_source_load_entries;

/// `build_source_load_entries` as a function of `module`.
pub fn build_source_load_entries_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_source_load_entries, module)
}

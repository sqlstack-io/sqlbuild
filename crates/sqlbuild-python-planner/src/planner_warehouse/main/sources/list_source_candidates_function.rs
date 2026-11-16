//! `list_source_candidates` as a function of `module`.

use crate::planner_warehouse::_helpers::sources::semantic_sources::list_source_candidates;

/// `list_source_candidates` as a function of `module`.
pub fn list_source_candidates_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(list_source_candidates, module)
}

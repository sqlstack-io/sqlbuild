//! `list_source_candidates` as a function of `module`.

use crate::planner_warehouse::_helpers::sources::semantic_sources;

/// `list_source_candidates` as a function of `module`.
#[inline]
pub fn list_source_candidates_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    semantic_sources::list_source_candidates_function(module)
}

//! `planner_metadata_name_filter` as a function of `module`.

use crate::planner_warehouse::_helpers::inspection::snapshot;

/// `planner_metadata_name_filter` as a function of `module`.
#[inline]
pub fn planner_metadata_name_filter_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    snapshot::planner_metadata_name_filter_function(module)
}

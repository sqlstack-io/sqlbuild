//! `gather_warehouse_snapshot` as a function of `module`.

use crate::planner_warehouse::_helpers::inspection::snapshot;

/// `gather_warehouse_snapshot` as a function of `module`.
#[inline]
pub fn gather_warehouse_snapshot_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    snapshot::gather_warehouse_snapshot_function(module)
}

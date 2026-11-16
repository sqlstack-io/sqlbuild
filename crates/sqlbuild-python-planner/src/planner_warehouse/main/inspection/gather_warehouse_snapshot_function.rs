//! `gather_warehouse_snapshot` as a function of `module`.

use crate::planner_warehouse::_helpers::inspection::snapshot::gather_warehouse_snapshot;

/// `gather_warehouse_snapshot` as a function of `module`.
pub fn gather_warehouse_snapshot_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(gather_warehouse_snapshot, module)
}

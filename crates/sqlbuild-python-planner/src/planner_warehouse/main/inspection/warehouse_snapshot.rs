//! Gather relations, columns, fingerprints and cursor bounds of the planned target schemas.

use crate::planner_warehouse::_helpers::inspection::snapshot;
use crate::planner_warehouse::models::SnapshotRequest;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Gather relations, columns, fingerprints and cursor bounds of the planned target schemas.
#[inline]
pub fn warehouse_snapshot<'py>(request: &SnapshotRequest<'py>) -> PyResult<Bound<'py, PyAny>> {
    snapshot::warehouse_snapshot(request)
}

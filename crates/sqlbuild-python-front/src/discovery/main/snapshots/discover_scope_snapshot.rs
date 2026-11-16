//! Parse the scope-relevant authored roots while retaining independent file faults.

use crate::discovery::_helpers::snapshots::scope_snapshot::tolerant_discovery;
use crate::discovery::_helpers::snapshots::snapshot::in_snapshot;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Parse the scope-relevant authored roots while retaining independent file faults.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn discover_scope_snapshot<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    in_snapshot(project_dir, || tolerant_discovery(project_dir))
}

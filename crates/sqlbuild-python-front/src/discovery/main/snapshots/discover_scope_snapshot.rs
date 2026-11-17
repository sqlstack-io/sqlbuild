//! Parse the scope-relevant authored roots while retaining independent file faults.

use crate::discovery::_helpers::snapshots::scope_snapshot;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Parse the scope-relevant authored roots while retaining independent file faults.
#[inline]
pub fn discover_scope_snapshot<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    scope_snapshot::discover_scope_snapshot(project_dir)
}

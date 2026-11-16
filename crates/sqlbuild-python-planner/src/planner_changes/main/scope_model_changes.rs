//! Scope model changes.

use crate::planner_changes::_helpers::detect;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// Scope model changes.
#[inline]
pub fn scope_model_changes<'py>(
    (project, scope, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    query_change_tracking: Option<bool>,
    (version_hashes, metadata_jsons): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyDict>> {
    detect::scope_model_changes(
        (project, scope, snapshot),
        query_change_tracking,
        (version_hashes, metadata_jsons),
    )
}

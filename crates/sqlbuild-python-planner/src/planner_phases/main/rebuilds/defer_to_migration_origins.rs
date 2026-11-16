//! Redirect unselected `models` unbuilt in the deferred target to their `migrate_from` relation.

use crate::planner_phases::_helpers::rebuilds::deferred_origins;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// Redirect unselected `models` unbuilt in the deferred target to their `migrate_from` relation.
#[inline]
pub fn defer_to_migration_origins<'py>(
    (project, adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (locations, relations): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
    models: &[String],
) -> PyResult<Vec<(String, String)>> {
    deferred_origins::defer_to_migration_origins(
        (project, adapter, connection),
        (locations, relations),
        models,
    )
}

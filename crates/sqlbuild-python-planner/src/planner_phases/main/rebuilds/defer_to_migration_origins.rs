//! Redirect unselected `models` unbuilt in the deferred target to their `migrate_from` relation.

use crate::planner_phases::_helpers::rebuilds::deferred_origins;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;
use sqlbuild_python_base::native_host::models::Dialect;

/// Redirect unselected `models` unbuilt in the deferred target to their `migrate_from` relation.
#[inline]
pub fn defer_to_migration_origins<'py>(
    (project, dialect, reads): (
        &Bound<'py, PyAny>,
        &Bound<'py, Dialect>,
        &Bound<'py, PlannerReads>,
    ),
    (locations, relations): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
    models: &[String],
) -> PyResult<Vec<(String, String)>> {
    deferred_origins::defer_to_migration_origins(
        (project, dialect, reads),
        (locations, relations),
        models,
    )
}

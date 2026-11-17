//! Redirect unselected `models` unbuilt in the deferred target to their `migrate_from` relation.

use crate::planner_phases::_helpers::rebuilds::deferred_origins;
<<<<<<< HEAD
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;
use sqlbuild_python_base::native_host::models::Dialect;
=======
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;
>>>>>>> origin/main

/// Redirect unselected `models` unbuilt in the deferred target to their `migrate_from` relation.
#[inline]
pub fn defer_to_migration_origins<'py>(
<<<<<<< HEAD
    (project, dialect, reads): (
        &Bound<'py, PyAny>,
        &Bound<'py, Dialect>,
        &Bound<'py, PlannerReads>,
    ),
=======
    (project, adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
>>>>>>> origin/main
    (locations, relations): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
    models: &[String],
) -> PyResult<Vec<(String, String)>> {
    deferred_origins::defer_to_migration_origins(
<<<<<<< HEAD
        (project, dialect, reads),
=======
        (project, adapter, connection),
>>>>>>> origin/main
        (locations, relations),
        models,
    )
}

//! Raise S405 when a selected resource reads an external source table that does not exist.

use crate::planner_warehouse::_helpers::sources::source_tables;
use pyo3::prelude::{Bound, PyAny, PyResult};
use crate::planner_warehouse::models::PlannerReads;
use sqlbuild_python_base::native_host::models::Dialect;

/// Raise S405 when a selected resource reads an external source table that does not exist.
#[inline]
pub fn check_selected_source_tables_exist<'py>(
    project: &Bound<'py, PyAny>,
    (dialect, reads): (&Bound<'py, Dialect>, &Bound<'py, PlannerReads>),
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<()> {
<<<<<<< ours
    source_tables::check_selected_source_tables_exist(project, (dialect, reads), scope, relations)
=======
    source_tables::check_selected_source_tables_exist(
        project,
        (adapter, connection),
        scope,
        relations,
    )
>>>>>>> theirs
}

//! Old-name compatibility views to create, resume or retire for the planned model and column migrations.

use crate::planner_migrations::_helpers::old_name::old_name_planning;
use crate::planner_migrations::models::MigrationPhase;
use crate::planner_migrations::models::OldNameViewPlanning;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Old-name compatibility views to create, resume or retire for the planned model and column migrations.
#[inline]
pub fn plan_old_name_views<'py>(
    phase: &MigrationPhase<'py>,
    moves: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<OldNameViewPlanning<'py>> {
    old_name_planning::plan_old_name_views(phase, moves)
}

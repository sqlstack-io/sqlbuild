//! Model migrations of the selected scope: discovered and declared moves, decided against live state.

use crate::planner_migrations::_helpers::model::model_planning;
use crate::planner_migrations::models::MigrationPhase;
use crate::planner_migrations::models::ModelMigrationPlanning;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Model migrations of the selected scope: discovered and declared moves, decided against live state.
#[inline]
pub fn plan_model_migrations<'py>(
    phase: &MigrationPhase<'py>,
    fingerprints: &Bound<'py, PyAny>,
) -> PyResult<ModelMigrationPlanning<'py>> {
    model_planning::plan_model_migrations(phase, fingerprints)
}

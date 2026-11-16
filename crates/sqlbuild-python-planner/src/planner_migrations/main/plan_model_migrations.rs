//! Model migrations of the selected scope: discovered and declared moves, decided against live state.

use crate::planner_migrations::_helpers::model::model_planning::discover;
use crate::planner_migrations::_helpers::model::model_planning::finish;
use crate::planner_migrations::models::{MigrationPhase, ModelMigrationPlanning};
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Model migrations of the selected scope: discovered and declared moves, decided against live state.
pub fn plan_model_migrations<'py>(
    phase: &MigrationPhase<'py>,
    fingerprints: &Bound<'py, PyAny>,
) -> PyResult<ModelMigrationPlanning<'py>> {
    let runtime = phase.runtime.clone();
    let snapshot = phase.snapshot.clone();
    let schemas =
        crate::planner_migrations::_helpers::planning::planning_origin::project_schemas(&runtime)?;
    let state = crate::planner_migrations::_helpers::discovery::state_inspection::inspection(
        &runtime,
        Some(&native_field(&snapshot, "fingerprints")?),
    )?;
    let discovery = discover((phase, fingerprints), &state, &schemas)?;
    finish(phase, &state, &schemas, &discovery)
}

//! Decide column renames for the selected incremental and snapshot models.

use crate::planner_migrations::_helpers::column::column_planning;
use crate::planner_migrations::models::{
    ColumnMigrationInputs, ColumnMigrationPlanning, MigrationPhase,
};
use pyo3::prelude::PyResult;

/// Decide column renames for the selected incremental and snapshot models.
#[inline]
pub fn plan_column_migrations<'py>(
    phase: &MigrationPhase<'py>,
    columns: &ColumnMigrationInputs<'py>,
) -> PyResult<ColumnMigrationPlanning<'py>> {
    column_planning::plan_column_migrations(phase, columns)
}

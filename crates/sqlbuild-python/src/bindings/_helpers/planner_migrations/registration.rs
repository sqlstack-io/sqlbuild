//! Register pure migration helpers.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use sqlbuild_python_planner::planner_migrations::main::register_column_planning::register_column_planning;
use sqlbuild_python_planner::planner_migrations::main::register_columns::register_columns;
use sqlbuild_python_planner::planner_migrations::main::register_fingerprint::register_fingerprint;
use sqlbuild_python_planner::planner_migrations::main::register_projections::register_projections;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_columns(module)?;
    register_column_planning(module)?;
    register_projections(module)?;
    module
        .add_class::<sqlbuild_python_planner::planner_migrations::models::MigrationStateInspection>(
        )?;
    register_fingerprint(module)
}

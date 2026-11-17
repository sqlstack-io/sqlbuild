//! Register the text and JSON plan output and the cursor safety serializers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_planner::plan_output::main::register_display::register_display;
use sqlbuild_python_planner::plan_output::main::register_migrations::register_migrations;
use sqlbuild_python_planner::plan_output::main::register_plan_json::register_plan_json;
use sqlbuild_python_planner::plan_output::main::register_plan_text::register_plan_text;
use sqlbuild_python_planner::plan_output::main::register_safety::register_safety;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_display(module)?;
    register_migrations(module)?;
    register_plan_json(module)?;
    register_plan_text(module)?;
    register_safety(module)
}

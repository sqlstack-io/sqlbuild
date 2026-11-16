//! Register planner change detection.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_planner::planner_changes::main::register_actions::register_actions;
use sqlbuild_python_planner::planner_changes::main::register_detect::register_detect;
use sqlbuild_python_planner::planner_changes::main::register_metadata::register_metadata;
use sqlbuild_python_planner::planner_changes::main::register_renames::register_renames;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_actions(module)?;
    register_detect(module)?;
    register_metadata(module)?;
    register_renames(module)
}

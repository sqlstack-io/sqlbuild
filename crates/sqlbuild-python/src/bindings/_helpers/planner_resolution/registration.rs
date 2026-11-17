//! Register planner SQL resolution entry points.

use pyo3::prelude::{Bound, PyModule, PyResult};
use sqlbuild_python_planner::planner_resolution::main::cursor::register_cursor_bounds::register_cursor_bounds;
use sqlbuild_python_planner::planner_resolution::main::cursor::register_cursor_overrides::register_cursor_overrides;
use sqlbuild_python_planner::planner_resolution::main::cursor::register_cursor_ranges::register_cursor_ranges;
use sqlbuild_python_planner::planner_resolution::main::cursor::register_cursor_warnings::register_cursor_warnings;
use sqlbuild_python_planner::planner_resolution::main::cursor::register_cursor_windows::register_cursor_windows;
use sqlbuild_python_planner::planner_resolution::main::references::register_locations::register_locations;
use sqlbuild_python_planner::planner_resolution::main::references::register_orchestration::register_orchestration;
use sqlbuild_python_planner::planner_resolution::main::references::register_refs::register_refs;
use sqlbuild_python_planner::planner_resolution::main::references::register_sources::register_sources;
use sqlbuild_python_planner::planner_resolution::main::safety::register_future_safety::register_future_safety;
use sqlbuild_python_planner::planner_resolution::main::safety::register_maximum_safety::register_maximum_safety;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_cursor_bounds(module)?;
    register_cursor_overrides(module)?;
    register_cursor_ranges(module)?;
    register_cursor_warnings(module)?;
    register_future_safety(module)?;
    register_maximum_safety(module)?;
    register_cursor_windows(module)?;
    register_refs(module)?;
    register_locations(module)?;
    register_orchestration(module)?;
    register_sources(module)
}

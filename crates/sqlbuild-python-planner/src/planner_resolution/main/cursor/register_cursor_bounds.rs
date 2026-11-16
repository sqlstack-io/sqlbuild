//! Register the cursor bounds functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_bounds::apply_typed_cursor_replay_policy;
use crate::planner_resolution::_helpers::cursor::cursor_bounds::compute_cursor_bounds;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cursor bounds functions and classes.
pub fn register_cursor_bounds(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compute_cursor_bounds, module)?)?;
    module.add_function(wrap_pyfunction!(apply_typed_cursor_replay_policy, module)?)
}

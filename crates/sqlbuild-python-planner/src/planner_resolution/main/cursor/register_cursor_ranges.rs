//! Register the cursor ranges functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_ranges::compute_microbatch_range;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::compute_plan_cursor_bounds;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::resolve_cursor_overrides;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cursor ranges functions and classes.
pub fn register_cursor_ranges(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compute_plan_cursor_bounds, module)?)?;
    module.add_function(wrap_pyfunction!(compute_microbatch_range, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_cursor_overrides, module)?)
}

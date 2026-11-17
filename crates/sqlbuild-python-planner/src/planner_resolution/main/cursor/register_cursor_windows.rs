//! Register the cursor windows functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_windows::cursor_window_override_flags;
use crate::planner_resolution::_helpers::cursor::cursor_windows::decide_empty_cursor_inputs;
use crate::planner_resolution::_helpers::cursor::cursor_windows::empty_cursor_window;
use crate::planner_resolution::_helpers::cursor::cursor_windows::empty_input_rebuild_refusal;
use crate::planner_resolution::_helpers::cursor::cursor_windows::normalize_cursor_snapshot_grain;
use crate::planner_resolution::main::cursor::resolve_effective_timestamp_grain::resolve_effective_timestamp_grain;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cursor windows functions and classes.
pub fn register_cursor_windows(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(decide_empty_cursor_inputs, module)?)?;
    module.add_function(wrap_pyfunction!(empty_cursor_window, module)?)?;
    module.add_function(wrap_pyfunction!(cursor_window_override_flags, module)?)?;
    module.add_function(wrap_pyfunction!(empty_input_rebuild_refusal, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_effective_timestamp_grain, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_cursor_snapshot_grain, module)?)
}

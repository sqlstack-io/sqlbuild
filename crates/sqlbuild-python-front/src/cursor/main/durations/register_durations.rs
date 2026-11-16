//! Register the durations functions and classes.

use crate::cursor::main::durations::parse_cursor_duration::parse_cursor_duration;
use crate::cursor::main::durations::shift_by_cursor_duration::shift_by_cursor_duration;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the durations functions and classes.
pub fn register_durations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_cursor_duration, module)?)?;
    module.add_function(wrap_pyfunction!(shift_by_cursor_duration, module)?)?;
    Ok(())
}

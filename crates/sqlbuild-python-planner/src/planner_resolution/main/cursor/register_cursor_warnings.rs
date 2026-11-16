//! Register the cursor warnings functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_warnings::future_cursor_cap_warning;
use crate::planner_resolution::_helpers::cursor::cursor_warnings::maximum_start_cap_warning;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cursor warnings functions and classes.
pub fn register_cursor_warnings(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(future_cursor_cap_warning, module)?)?;
    module.add_function(wrap_pyfunction!(maximum_start_cap_warning, module)?)
}

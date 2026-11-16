//! Register the cursor types functions and classes.

use crate::planner_strategy::_helpers::cursor_types::check_cursor_type_consistency;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cursor types functions and classes.
pub fn register_cursor_types(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(check_cursor_type_consistency, module)?)
}

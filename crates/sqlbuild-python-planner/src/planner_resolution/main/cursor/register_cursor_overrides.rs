//! Register the cursor overrides functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_overrides::type_cursor_overrides_of;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cursor overrides functions and classes.
pub fn register_cursor_overrides(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(type_cursor_overrides_of, module)?)
}

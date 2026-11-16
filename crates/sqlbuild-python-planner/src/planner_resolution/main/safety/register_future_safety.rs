//! Register the future safety functions and classes.

use crate::planner_resolution::_helpers::safety::future_safety::resolve_future_cursor_safety;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the future safety functions and classes.
pub fn register_future_safety(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_future_cursor_safety, module)?)
}

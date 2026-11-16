//! Register the clone plans functions and classes.

use crate::planner_entries::_helpers::plans::clone_plans::resolve_clone_boundary;
use crate::planner_entries::_helpers::plans::clone_plans::resolve_skipped_view_chain;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the clone plans functions and classes.
pub fn register_clone_plans(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_clone_boundary, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_skipped_view_chain, module)?)
}

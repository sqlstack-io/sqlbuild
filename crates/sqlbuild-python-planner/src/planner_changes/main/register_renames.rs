//! Register the renames functions and classes.

use crate::planner_changes::_helpers::renames::origin_reference_names_of;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the renames functions and classes.
pub fn register_renames(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(origin_reference_names_of, module)?)
}

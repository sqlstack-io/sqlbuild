//! Register the plan json functions and classes.

use crate::plan_output::main::format_plan_json::format_plan_json;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the plan json functions and classes.
pub fn register_plan_json(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_plan_json, module)?)
}

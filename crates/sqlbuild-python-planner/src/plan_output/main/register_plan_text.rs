//! Register the plan text functions and classes.

use crate::plan_output::main::format_plan::format_plan;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the plan text functions and classes.
pub fn register_plan_text(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_plan, module)?)
}

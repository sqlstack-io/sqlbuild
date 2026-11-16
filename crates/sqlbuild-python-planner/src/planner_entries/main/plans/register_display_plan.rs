//! Register the display plan functions and classes.

use crate::planner_entries::main::plans::build_display_only_sqlbuild_plan::build_display_only_sqlbuild_plan;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the display plan functions and classes.
pub fn register_display_plan(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_display_only_sqlbuild_plan, module)?)
}

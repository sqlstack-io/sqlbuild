//! Add the execution plan entry point.

use crate::planner_entries::main::plans::build_execution_plan::build_execution_plan;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Add the execution plan entry point.
pub fn register_execution_plan(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_execution_plan, module)?)
}

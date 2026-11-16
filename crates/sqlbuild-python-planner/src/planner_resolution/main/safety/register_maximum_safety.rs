//! Register the maximum safety functions and classes.

use crate::planner_resolution::_helpers::safety::maximum_safety::apply_maximum_start_policy;
use crate::planner_resolution::main::safety::maximum_allowed_start::maximum_allowed_start;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the maximum safety functions and classes.
pub fn register_maximum_safety(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(maximum_allowed_start, module)?)?;
    module.add_function(wrap_pyfunction!(apply_maximum_start_policy, module)?)
}

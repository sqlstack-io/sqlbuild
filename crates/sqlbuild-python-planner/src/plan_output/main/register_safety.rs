//! Register the safety functions and classes.

use crate::plan_output::_helpers::safety::serialize_maximum_start_safety;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the safety functions and classes.
pub fn register_safety(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(serialize_maximum_start_safety, module)?)
}

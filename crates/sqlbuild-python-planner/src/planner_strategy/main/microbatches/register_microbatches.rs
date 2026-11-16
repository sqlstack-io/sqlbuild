//! Register the microbatches functions and classes.

use crate::planner_strategy::_helpers::microbatches::microbatch_limit_warning;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the microbatches functions and classes.
pub fn register_microbatches(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(microbatch_limit_warning, module)?)?;
    Ok(())
}

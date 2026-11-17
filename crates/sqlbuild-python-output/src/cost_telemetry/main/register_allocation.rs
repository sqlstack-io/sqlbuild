//! Register the allocation functions and classes.

use crate::cost_telemetry::_helpers::allocation::allocate_run_cost;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the allocation functions and classes.
pub fn register_allocation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(allocate_run_cost, module)?)?;
    Ok(())
}

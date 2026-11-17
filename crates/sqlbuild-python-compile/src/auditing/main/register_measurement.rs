//! Register the measurement functions and classes.

use crate::auditing::_helpers::measurement::evaluate_measurement;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the measurement functions and classes.
pub fn register_measurement(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(evaluate_measurement, module)?)?;
    Ok(())
}

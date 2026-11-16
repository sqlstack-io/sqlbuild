//! Register the raised functions and classes.

use crate::discovery::_helpers::failures::raised::_oracle_read_error;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the raised functions and classes.
pub fn register_raised(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(_oracle_read_error, module)?)
}

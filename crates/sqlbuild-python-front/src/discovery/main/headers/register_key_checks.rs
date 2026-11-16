//! Register the key checks functions and classes.

use crate::discovery::_helpers::headers::key_checks::unknown_mapping_keys;
use crate::discovery::_helpers::headers::key_checks::unsupported_keys_help;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the key checks functions and classes.
pub fn register_key_checks(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(unknown_mapping_keys, module)?)?;
    module.add_function(wrap_pyfunction!(unsupported_keys_help, module)?)
}

//! Register the custom rule cache functions and classes.

use crate::rules::_helpers::custom::custom_rule_cache::custom_rule_identity;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the custom rule cache functions and classes.
pub fn register_custom_rule_cache(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(custom_rule_identity, module)?)?;
    Ok(())
}

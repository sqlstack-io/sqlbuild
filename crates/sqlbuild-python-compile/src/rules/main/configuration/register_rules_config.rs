//! Register the rules config functions and classes.

use crate::rules::main::configuration::load_rules_config::load_rules_config;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the rules config functions and classes.
pub fn register_rules_config(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_rules_config, module)?)?;
    Ok(())
}

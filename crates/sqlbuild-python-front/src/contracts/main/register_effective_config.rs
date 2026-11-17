//! Register the effective config functions and classes.

use crate::contracts::_helpers::effective_config::effective_config_connection;
use crate::contracts::_helpers::effective_config::effective_config_target_fields;
use crate::contracts::_helpers::effective_config::effective_config_target_name;
use crate::contracts::main::effective_config_settings::effective_config_settings;
use crate::contracts::main::effective_config_vars::effective_config_vars;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the effective config functions and classes.
pub fn register_effective_config(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(effective_config_target_name, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_target_fields, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_vars, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_connection, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_settings, module)?)?;
    Ok(())
}

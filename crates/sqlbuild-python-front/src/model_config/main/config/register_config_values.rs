//! Register the config values functions and classes.

use crate::model_config::_helpers::config::config_values::config_bool;
use crate::model_config::_helpers::config::config_values::config_cursor_bound;
use crate::model_config::_helpers::config::config_values::config_int;
use crate::model_config::_helpers::config::config_values::config_string_tuple;
use crate::model_config::main::config::config_str::config_str;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the config values functions and classes.
pub fn register_config_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(config_str, module)?)?;
    module.add_function(wrap_pyfunction!(config_bool, module)?)?;
    module.add_function(wrap_pyfunction!(config_string_tuple, module)?)?;
    module.add_function(wrap_pyfunction!(config_int, module)?)?;
    module.add_function(wrap_pyfunction!(config_cursor_bound, module)?)?;
    Ok(())
}

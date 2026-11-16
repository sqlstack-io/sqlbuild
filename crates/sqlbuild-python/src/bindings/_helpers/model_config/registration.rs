//! Register the native model configuration functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::model_config::main::config::register_config_errors::register_config_errors;
use sqlbuild_python_front::model_config::main::config::register_config_values::register_config_values;
use sqlbuild_python_front::model_config::main::config::register_template_data::register_template_data;
use sqlbuild_python_front::model_config::main::headers::register_header_columns::register_header_columns;
use sqlbuild_python_front::model_config::main::model_nodes::register_model_config_builder::register_model_config_builder;
use sqlbuild_python_front::model_config::main::model_nodes::register_model_validator::register_model_validator;
use sqlbuild_python_front::model_config::main::model_nodes::register_unicode_environment::register_unicode_environment;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_config_errors(module)?;
    register_config_values(module)?;
    register_header_columns(module)?;
    register_model_config_builder(module)?;
    register_model_validator(module)?;
    register_template_data(module)?;
    register_unicode_environment(module)?;
    Ok(())
}

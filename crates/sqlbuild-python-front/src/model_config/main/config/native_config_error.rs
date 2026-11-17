//! Wrap `error` for Python.

use crate::model_config::_helpers::config::config_errors::COMPILE_INPUT_CLASS;
use crate::model_config::_helpers::config::config_errors::CONFIG_VALUE_TYPE_CLASS;
use crate::model_config::_helpers::config::config_errors::DISCOVERY_CONFLICT_CLASS;
use crate::model_config::_helpers::config::config_errors::RESOURCE_IDENTITY_CLASS;
use crate::model_config::errors::NativeConfigError;
use pyo3::Py;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_model_config::errors::{ConfigError, ErrorClass};

/// Wrap `error` for Python.
pub fn native_config_error(py: Python<'_>, error: ConfigError) -> PyResult<Py<NativeConfigError>> {
    let class_name = match error.class {
        ErrorClass::CompileInput => COMPILE_INPUT_CLASS,
        ErrorClass::ConfigValueType => CONFIG_VALUE_TYPE_CLASS,
        ErrorClass::ResourceIdentity => RESOURCE_IDENTITY_CLASS,
        ErrorClass::DiscoveryConflict => DISCOVERY_CONFLICT_CLASS,
    };
    Py::new(
        py,
        NativeConfigError {
            class_name,
            message: error.message,
            code: error.code,
            help: error.help,
            key: error.key,
        },
    )
}

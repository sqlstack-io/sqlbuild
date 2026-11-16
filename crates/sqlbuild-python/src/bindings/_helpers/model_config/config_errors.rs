//! Native config errors handed to Python, which raises them as the Python stages would.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{Py, PyErr, pymethods};
use sqlbuild_model_config::errors::{ConfigError, ErrorClass};

use crate::bindings::_helpers::model_config::config_values::config_value_type_error;
use crate::bindings::_helpers::model_config::errors::NativeConfigError;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

const EXPECTED_STRING: &str = "a string";

const COMPILE_INPUT_CLASS: &str = "compile_input";
const CONFIG_VALUE_TYPE_CLASS: &str = "config_value_type";
const RESOURCE_IDENTITY_CLASS: &str = "resource_identity";
const DISCOVERY_CONFLICT_CLASS: &str = "discovery_conflict";

/// Wrap `error` for Python.
pub(crate) fn native_config_error(
    py: Python<'_>,
    error: ConfigError,
) -> PyResult<Py<NativeConfigError>> {
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

/// The Python exception a native config error raises as, given the values it names.
pub(crate) fn config_exception<'py>(
    error: &Bound<'py, PyAny>,
    values: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let error: Bound<'py, NativeConfigError> = error.cast::<NativeConfigError>()?.clone();
    error.get().exception(error.py(), values)
}

/// `config_exception` as the raised error, or the failure building it.
pub(crate) fn raised_config_exception(
    error: &Bound<'_, PyAny>,
    values: Option<&Bound<'_, PyAny>>,
) -> PyErr {
    match config_exception(error, values) {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

#[pymethods]
impl NativeConfigError {
    /// The exception this error raises as: its class, message, code and help.
    #[pyo3(signature = (values = None))]
    fn exception<'py>(
        &self,
        py: Python<'py>,
        values: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if self.class_name == CONFIG_VALUE_TYPE_CLASS
            && let (Some(key), Some(values)) = (&self.key, values)
        {
            return config_value_type_error(
                key,
                EXPECTED_STRING,
                &values.get_item(key)?.get_type().into_any(),
            );
        }
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("help", &self.help)?;
        let class: ErrorId = match self.class_name {
            RESOURCE_IDENTITY_CLASS => ErrorId::ResourceIdentity,
            DISCOVERY_CONFLICT_CLASS => ErrorId::DiscoveryConflict,
            _ => {
                keywords.set_item("code", self.code)?;
                ErrorId::CompileInput
            }
        };
        error_class(py, class)?.call((&self.message,), Some(&keywords))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeConfigError>()?;
    Ok(())
}

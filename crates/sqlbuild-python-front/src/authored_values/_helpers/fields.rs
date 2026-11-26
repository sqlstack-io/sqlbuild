//! Optional authored string and boolean settings, rejected with the caller's error class.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyString, PyStringMethods};

use sqlbuild_python_base::boundary::models::Request;

/// The setting a validator reads and how it names the setting in errors.
struct Setting<'py> {
    raw_value: Bound<'py, PyAny>,
    file_path: Bound<'py, PyAny>,
    label: String,
    key: String,
    error_class: Bound<'py, PyAny>,
}

impl<'py> Setting<'py> {
    fn read(request: &Request<'_, 'py>) -> PyResult<Self> {
        Ok(Self {
            raw_value: request.get("raw_value")?,
            file_path: request.get("file_path")?,
            label: request.get("label")?.extract()?,
            key: request.get("key")?.extract()?,
            error_class: request.get("error_class")?,
        })
    }

    /// `error_class(f"{file_path} {label} '{key}' {problem}")`.
    fn rejected(&self, problem: &str) -> PyErr {
        let message: String = match self.file_path.str() {
            Ok(path) => format!("{path} {} '{}' {problem}", self.label, self.key),
            Err(error) => return error,
        };
        match self.error_class.call1((message,)) {
            Ok(error) => PyErr::from_value(error),
            Err(error) => error,
        }
    }
}

/// An optional non-empty string setting, or `None` when absent.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn optional_named_string<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let setting: Setting<'py> = Setting::read(&Request::new("optional_named_string", request)?)?;
    if setting.raw_value.is_none() {
        return Ok(setting.raw_value);
    }
    let blank: bool = match setting.raw_value.cast::<PyString>() {
        Ok(text) => text.to_cow()?.trim().is_empty(),
        Err(_) => true,
    };
    if blank {
        return Err(setting.rejected("must be a non-empty string"));
    }
    Ok(setting.raw_value)
}

/// An optional boolean setting, or `default` when absent.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn optional_named_bool<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("optional_named_bool", request)?;
    let setting: Setting<'py> = Setting::read(&request)?;
    if setting.raw_value.is_none() {
        return request.get("default");
    }
    if !setting.raw_value.is_instance_of::<PyBool>() {
        return Err(setting.rejected("must be a boolean"));
    }
    Ok(setting.raw_value)
}

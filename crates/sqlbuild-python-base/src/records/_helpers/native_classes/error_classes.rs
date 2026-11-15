//! The native constructor shared by the expected CLI and migration-state error classes.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods, PyTypeMethods};
use pyo3::{IntoPyObjectExt, pyclass, pymethods};

use crate::records::_helpers::native_classes::fields::field as native_field;
use crate::records::constants::{ERROR_SPEC_ATTRIBUTE, ERRORS};
use crate::records::models::{ClassValue, ErrorSpec};

/// The `__init__` of the expected-error classes: `Error(message, *, code=None, help=None, **fields)`.
#[derive(Debug)]
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ErrorInitializer {}

#[pymethods]
impl ErrorInitializer {
    fn __get__(
        slf: &Bound<'_, Self>,
        instance: &Bound<'_, PyAny>,
        _owner: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if instance.is_none() {
            return Ok(slf.clone().into_any().unbind());
        }
        Ok(Py::new(
            slf.py(),
            BoundErrorInitializer {
                error: instance.clone().unbind(),
            },
        )?
        .into_any())
    }
}

/// `ErrorInitializer` bound to one error instance.
#[derive(Debug)]
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct BoundErrorInitializer {
    error: Py<PyAny>,
}

#[pymethods]
impl BoundErrorInitializer {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let error: &Bound<'_, PyAny> = self.error.bind(args.py());
        let name: String = error.get_type().qualname()?.to_string();
        let fields: Bound<'_, PyDict> = match kwargs {
            Some(kwargs) => kwargs.copy()?,
            None => PyDict::new(args.py()),
        };
        let message: Bound<'_, PyAny> = match (args.len(), fields.get_item("message")?) {
            (1, None) => args.get_item(0)?,
            (0, Some(message)) => {
                fields.del_item("message")?;
                message
            }
            (0, None) => {
                return Err(PyTypeError::new_err(format!(
                    "{name}.__init__() missing 1 required positional argument: 'message'"
                )));
            }
            (1, Some(_)) => {
                return Err(PyTypeError::new_err(format!(
                    "{name}.__init__() got multiple values for argument 'message'"
                )));
            }
            (count, _) => {
                return Err(PyTypeError::new_err(format!(
                    "{name}.__init__() takes 2 positional arguments but {} were given",
                    count + 1
                )));
            }
        };
        let code: Option<Bound<'_, PyAny>> = taken(&fields, "code")?;
        let help: Option<Bound<'_, PyAny>> = taken(&fields, "help")?;
        initialize(
            error,
            (&message, code.as_ref(), help.as_ref()),
            Some(&fields),
        )
    }
}

/// Remove and return the keyword `name`, if it was passed.
fn taken<'py>(fields: &Bound<'py, PyDict>, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value: Option<Bound<'py, PyAny>> = fields.get_item(name)?;
    if value.is_some() {
        fields.del_item(name)?;
    }
    Ok(value)
}

type ErrorArguments<'a, 'py> = (
    &'a Bound<'py, PyAny>,
    Option<&'a Bound<'py, PyAny>>,
    Option<&'a Bound<'py, PyAny>>,
);

/// Set `args`, `message`, `code`, `help` and the class's keyword fields on `error`.
fn initialize(
    error: &Bound<'_, PyAny>,
    (message, code, help): ErrorArguments<'_, '_>,
    fields: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = error.py();
    let class = error.get_type();
    let spec: &ErrorSpec = error_spec(&class.getattr(ERROR_SPEC_ATTRIBUTE)?.extract()?)?;
    let name: String = class.qualname()?.to_string();
    if let Some(fields) = fields {
        for key in fields.keys() {
            let key: String = key.extract()?;
            if !spec.required.contains(&key.as_str()) && !spec.optional.contains(&key.as_str()) {
                return Err(PyTypeError::new_err(format!(
                    "{name}.__init__() got an unexpected keyword argument '{key}'"
                )));
            }
        }
    }
    error.setattr("args", PyTuple::new(py, [message])?)?;
    error.setattr("message", message)?;
    match code {
        Some(code) if !code.is_none() => error.setattr("code", code)?,
        _ => {
            if let Ok(code) = native_field(&class, "code") {
                error.setattr("code", code)?;
            }
        }
    }
    error.setattr(
        "help",
        help.map_or_else(|| py.None().into_bound(py), Clone::clone),
    )?;
    for required in spec.required {
        let value = match fields {
            Some(fields) => fields.get_item(required)?,
            None => None,
        };
        let Some(value) = value else {
            return Err(PyTypeError::new_err(format!(
                "{name}.__init__() missing 1 required keyword-only argument: '{required}'"
            )));
        };
        error.setattr(*required, value)?;
    }
    for optional in spec.optional {
        let value = match fields {
            Some(fields) => fields.get_item(optional)?,
            None => None,
        };
        error.setattr(*optional, value.unwrap_or_else(|| py.None().into_bound(py)))?;
    }
    for (attribute, value) in spec.fixed {
        error.setattr(*attribute, class_value(py, *value)?)?;
    }
    Ok(())
}

fn error_spec(index: &usize) -> PyResult<&'static ErrorSpec> {
    ERRORS
        .iter()
        .flat_map(|group| group.iter())
        .nth(*index)
        .ok_or_else(|| PyTypeError::new_err("unknown native error class"))
}

/// The Python value of one class-level constant.
pub(crate) fn class_value(py: Python<'_>, value: ClassValue) -> PyResult<Bound<'_, PyAny>> {
    match value {
        ClassValue::Text(text) => text.into_bound_py_any(py),
        ClassValue::Int(number) => number.into_bound_py_any(py),
    }
}

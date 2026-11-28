//! Python audit argument values read as plain argument values.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyBool, PyFloat, PyInt, PyList, PyListMethods, PyString, PyStringMethods, PyTuple,
    PyTupleMethods,
};
use sqlbuild_attachments::audits::models::ArgumentValue;

/// One plain value; text with lone surrogates, rejected where it enters a compile, reads lossily.
pub(crate) fn argument_value(value: &Bound<'_, PyAny>) -> PyResult<ArgumentValue> {
    if value.is_none() {
        return Ok(ArgumentValue::Null);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(ArgumentValue::Boolean(value.extract()?));
    }
    if value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>() {
        return Ok(ArgumentValue::Number(
            value.str()?.to_string_lossy().into_owned(),
        ));
    }
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(ArgumentValue::Text(text.to_string_lossy().into_owned()));
    }
    if let Ok(items) = value.cast::<PyList>() {
        return Ok(ArgumentValue::List(
            items
                .iter()
                .map(|item| argument_value(&item))
                .collect::<PyResult<_>>()?,
        ));
    }
    if let Ok(items) = value.cast::<PyTuple>() {
        return Ok(ArgumentValue::Tuple(
            items
                .iter()
                .map(|item| argument_value(&item))
                .collect::<PyResult<_>>()?,
        ));
    }
    Ok(ArgumentValue::Opaque)
}

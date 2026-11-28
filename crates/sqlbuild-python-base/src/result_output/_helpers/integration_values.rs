//! Python values as integration JSON, and contract failures as `ObservabilityValidationError`.

use crate::native_host::main::host_error::host_error;
use crate::result_output::types::Checked;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString,
    PyStringMethods, PyTuple,
};
use sqlbuild_core::integration_results::errors::IntegrationRecordError;
use sqlbuild_core::integration_results::main::encode_integration_value::encode_integration_value;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};
use sqlbuild_host::entries::models::ExecutorHost;

/// The `ObservabilityValidationError` Python raised for the same contract failure.
pub(crate) fn validation_error(py: Python<'_>, error: &IntegrationRecordError) -> PyErr {
    host_error(
        py,
        ExecutorHost::ObservabilityValidationError,
        error.message.as_str(),
    )
}

/// A plain JSON Python value, or `None` when it holds something JSON cannot carry.
pub(crate) fn json_value(value: &Bound<'_, PyAny>) -> PyResult<Option<JsonValue>> {
    if value.is_none() {
        return Ok(Some(JsonValue::Null));
    }
    if let Ok(flag) = value.cast::<PyBool>() {
        return Ok(Some(JsonValue::Bool(flag.is_true())));
    }
    if value.is_instance_of::<PyInt>() {
        let decimal: String = value.str()?.to_string();
        return Ok(JsonInteger::parse(&decimal).map(JsonValue::Integer));
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(Some(JsonValue::Float(value.extract()?)));
    }
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(Some(JsonValue::String(text.to_string_lossy().into_owned())));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            let Some(item) = json_value(&item?)? else {
                return Ok(None);
            };
            items.push(item);
        }
        return Ok(Some(JsonValue::Array(items)));
    }
    if let Ok(mapping) = value.cast::<PyDict>() {
        let mut entries: Vec<(String, JsonValue)> = Vec::new();
        for (key, item) in mapping.iter() {
            let Ok(key) = key.cast::<PyString>() else {
                return Ok(None);
            };
            let Some(item) = json_value(&item)? else {
                return Ok(None);
            };
            entries.push((key.to_string_lossy().into_owned(), item));
        }
        return Ok(Some(JsonValue::Object(entries)));
    }
    Ok(None)
}

/// A plain JSON Python value, or the contract failure of a non-JSON value.
pub(crate) fn required_json(value: &Bound<'_, PyAny>) -> Checked<JsonValue> {
    Ok(json_value(value)?.ok_or_else(|| IntegrationRecordError {
        message: "integration value is not JSON".to_owned(),
    }))
}

/// `_bounded_metadata`: the value when it encodes within the metadata bounds, else `{}`.
pub(crate) fn bounded_metadata(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    let empty: JsonValue = JsonValue::Object(Vec::new());
    let Some(converted) = json_value(value)? else {
        return Ok(empty);
    };
    if encode_integration_value(&converted, false).is_err() {
        return Ok(empty);
    }
    Ok(converted)
}

/// `value` when it is a string, else `null`.
pub(crate) fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(JsonValue::String(text.to_string_lossy().into_owned()));
    }
    Ok(JsonValue::Null)
}

/// `str(value or "")`.
pub(crate) fn text_or_empty(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if !value.is_truthy()? {
        return Ok(JsonValue::String(String::new()));
    }
    Ok(JsonValue::String(
        value.str()?.to_string_lossy().into_owned(),
    ))
}

/// A JSON string.
pub(crate) fn text(value: &str) -> JsonValue {
    JsonValue::String(value.to_owned())
}

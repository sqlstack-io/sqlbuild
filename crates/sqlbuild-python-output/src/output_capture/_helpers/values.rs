//! Command-output text and JSON values across the Python boundary.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyUnicodeEncodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{
    PyBool, PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString,
    PyStringMethods, PyTuple, PyTypeMethods,
};
use sqlbuild_core::command_output::errors::UnencodableText;
use sqlbuild_core::command_output::types::PassText;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_front::macro_bridge::main::text::text;

const SURROGATE_PASS: &str = "surrogatepass";
const UTF8: &str = "utf-8";

/// The exception class `name` of the output capture exceptions module, raised with `message`.
pub(crate) fn output_error(py: Python<'_>, name: ExecutorHost, message: &str) -> PyErr {
    host_error(py, name, message)
}

/// `CommandOutputValidationError(message)`.
pub(crate) fn validation_error(py: Python<'_>, message: &str) -> PyErr {
    output_error(py, ExecutorHost::CommandOutputValidationError, message)
}

/// A Python `str` as `surrogatepass` UTF-8 bytes.
pub(crate) fn pass_text(value: &Bound<'_, PyString>) -> PyResult<PassText> {
    if let Ok(text) = value.to_str() {
        return Ok(text.as_bytes().to_vec());
    }
    Ok(value
        .call_method1("encode", (UTF8, SURROGATE_PASS))?
        .cast_into::<PyBytes>()?
        .as_bytes()
        .to_vec())
}

/// `surrogatepass` UTF-8 bytes as a Python `str`.
pub(crate) fn python_text<'py>(py: Python<'py>, value: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    match std::str::from_utf8(value) {
        Ok(text) => Ok(PyString::new(py, text).into_any()),
        Err(_) => PyBytes::new(py, value).call_method1("decode", (UTF8, SURROGATE_PASS)),
    }
}

/// The `UnicodeEncodeError` Python raises encoding `text` with a lone surrogate.
pub(crate) fn unencodable(py: Python<'_>, text: &[u8], error: &UnencodableText) -> PyErr {
    let raised = python_text(py, text).and_then(|text| {
        py.get_type::<PyUnicodeEncodeError>().call1((
            UTF8,
            text,
            error.position,
            error.position + 1,
            "surrogates not allowed",
        ))
    });
    match raised {
        Ok(exception) => PyErr::from_value(exception),
        Err(failure) => failure,
    }
}

fn type_name(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.get_type().name()?.to_string())
}

/// `freeze_command_output_json`: validate and recursively freeze a JSON-compatible value.
pub(crate) fn freeze<'py>(value: &Bound<'py, PyAny>, path: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value.is_none()
        || value.is_instance_of::<PyString>()
        || value.is_instance_of::<PyBool>()
        || value.is_instance_of::<PyInt>()
    {
        return Ok(value.clone());
    }
    if value.is_instance_of::<PyFloat>() {
        if !value.extract::<f64>()?.is_finite() {
            return Err(validation_error(
                py,
                &format!("{path} must not contain NaN or infinity"),
            ));
        }
        return Ok(value.clone());
    }
    if value.is_instance(&imported(py, "collections.abc", "Mapping")?)? {
        let frozen: Bound<'py, PyDict> = PyDict::new(py);
        for item in mapping_items(value)?.try_iter()? {
            let (key, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
            let Ok(key_text) = key.cast::<PyString>() else {
                return Err(validation_error(
                    py,
                    &format!("{path} keys must be strings"),
                ));
            };
            let child = format!("{path}.{}", key_text.to_cow()?);
            frozen.set_item(&key, freeze(&item, &child)?)?;
        }
        return imported(py, "types", "MappingProxyType")?.call1((frozen,));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        for (index, item) in value.try_iter()?.enumerate() {
            items.push(freeze(&item?, &format!("{path}[{index}]"))?);
        }
        return Ok(PyTuple::new(py, items)?.into_any());
    }
    Err(validation_error(
        py,
        &format!(
            "{path} contains non-JSON value of type {}",
            type_name(value)?
        ),
    ))
}

/// A frozen command-output JSON value for the native emitter.
pub(crate) fn json_value(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if value.is_none() {
        return Ok(JsonValue::Null);
    }
    if let Ok(flag) = value.cast::<PyBool>() {
        return Ok(JsonValue::Bool(flag.is_truthy()?));
    }
    if value.is_instance_of::<PyInt>() {
        let decimal: String = value.str()?.extract()?;
        return JsonInteger::parse(&decimal)
            .map(JsonValue::Integer)
            .ok_or_else(|| PyValueError::new_err("command output integer is not JSON"));
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(JsonValue::Float(value.extract::<f64>()?));
    }
    if let Ok(string) = value.cast::<PyString>() {
        return text(string);
    }
    if value.is_instance_of::<PyTuple>() || value.is_instance_of::<PyList>() {
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            items.push(json_value(&item?)?);
        }
        return Ok(JsonValue::Array(items));
    }
    let mut entries: Vec<(String, JsonValue)> = Vec::new();
    for item in mapping_items(value)?.try_iter()? {
        let (key, item): (String, Bound<'_, PyAny>) = item?.extract()?;
        entries.push((key, json_value(&item)?));
    }
    Ok(JsonValue::Object(entries))
}

/// `value` as an `int` other than `bool`, saturated to `i64`, or `None`.
pub(crate) fn plain_int(value: &Bound<'_, PyAny>) -> Option<i64> {
    if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
        return None;
    }
    let saturated = match value.gt(0) {
        Ok(true) => i64::MAX,
        _ => i64::MIN,
    };
    Some(value.extract::<i64>().unwrap_or(saturated))
}

/// A Rust value as a Python object.
pub(crate) fn object<'py, T: IntoPyObjectExt<'py>>(
    py: Python<'py>,
    value: T,
) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

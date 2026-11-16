//! Python values as `json.dumps` would encode them, optionally after `dataclasses.asdict`.

use crate::boundary::main::utf16_units::utf16_units;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString,
    PyStringMethods, PyTuple, PyTypeMethods,
};
use sqlbuild_analysis::python_values::models::{PythonKey, PythonValue};
use sqlbuild_core::json::models::JsonInteger;

use crate::boundary::main::native_compiler_error::native_compiler_error;
use crate::compile_outputs::main::float_key::float_key;
use crate::records::main::fields::field::field as native_field;

const DATACLASS_FIELDS: &str = "__dataclass_fields__";
const SURROGATE_KEY: &str = "a JSON artifact mapping key holds a lone surrogate, which the native \
                             encoder does not write";
const DATACLASSES_MODULE: &str = "dataclasses";

fn type_name(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.get_type().name()?.to_string())
}

/// A `str`, as UTF-16 code units when it holds lone surrogates.
pub(crate) fn text_value(text: &Bound<'_, PyString>) -> PyResult<PythonValue> {
    match text.to_str() {
        Ok(text) => Ok(PythonValue::String(text.to_owned())),
        Err(_) => utf16_units(text).map(PythonValue::Utf16),
    }
}

fn integer_value(value: &Bound<'_, PyAny>) -> PyResult<PythonValue> {
    if let Ok(signed) = value.extract::<i64>() {
        return Ok(PythonValue::Integer(JsonInteger::from(signed)));
    }
    if let Ok(unsigned) = value.extract::<u64>() {
        return Ok(PythonValue::Integer(JsonInteger::from(unsigned)));
    }
    let decimal: String = value.str()?.to_str()?.to_owned();
    Ok(JsonInteger::parse(&decimal).map_or(PythonValue::Null, PythonValue::Integer))
}

/// A mapping key as `json.dumps` writes it, or its `TypeError` message.
fn key_value(key: &Bound<'_, PyAny>) -> PyResult<PythonKey> {
    if let Ok(text) = key.downcast::<PyString>() {
        return text
            .to_str()
            .map(|text| PythonKey {
                text: Ok(text.to_owned()),
                is_str: true,
            })
            .map_err(|_| native_compiler_error(SURROGATE_KEY));
    }
    Ok(PythonKey {
        text: scalar_key_text(key)?,
        is_str: false,
    })
}

fn scalar_key_text(key: &Bound<'_, PyAny>) -> PyResult<Result<String, String>> {
    if key.is_none() {
        return Ok(Ok("null".to_owned()));
    }
    if let Ok(flag) = key.downcast::<PyBool>() {
        return Ok(Ok(if flag.is_true() { "true" } else { "false" }.to_owned()));
    }
    if key.is_instance_of::<PyInt>() {
        return Ok(Ok(key.str()?.to_str()?.to_owned()));
    }
    if key.is_instance_of::<PyFloat>() {
        return float_key(key).map(Ok);
    }
    Ok(Err(format!(
        "keys must be str, int, float, bool or None, not {}",
        type_name(key)?
    )))
}

fn dataclass_value(value: &Bound<'_, PyAny>) -> PyResult<PythonValue> {
    let fields: Bound<'_, PyAny> = value
        .py()
        .import(DATACLASSES_MODULE)?
        .getattr("fields")?
        .call1((value,))?;
    let mut entries: Vec<(PythonKey, PythonValue)> = Vec::new();
    for field in fields.try_iter()? {
        let name: Bound<'_, PyAny> = native_field(&field?, "name")?;
        let item: Bound<'_, PyAny> = value.getattr(name.downcast::<PyString>()?)?;
        entries.push((key_value(&name)?, python_value(&item, true)?));
    }
    Ok(PythonValue::Object(entries))
}

/// The value `json.dumps` encodes, copied by `asdict` first when `dataclasses` is set.
pub(crate) fn python_value(value: &Bound<'_, PyAny>, dataclasses: bool) -> PyResult<PythonValue> {
    if value.is_none() {
        return Ok(PythonValue::Null);
    }
    if dataclasses && value.get_type().hasattr(DATACLASS_FIELDS)? {
        return dataclass_value(value);
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        return Ok(PythonValue::Array(
            value
                .try_iter()?
                .map(|item| python_value(&item?, dataclasses))
                .collect::<PyResult<_>>()?,
        ));
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        return Ok(PythonValue::Object(
            mapping
                .iter()
                .map(|(key, item)| Ok((key_value(&key)?, python_value(&item, dataclasses)?)))
                .collect::<PyResult<_>>()?,
        ));
    }
    if let Ok(flag) = value.downcast::<PyBool>() {
        return Ok(PythonValue::Bool(flag.is_true()));
    }
    if value.is_instance_of::<PyInt>() {
        return integer_value(value);
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(PythonValue::Float(value.extract::<f64>()?));
    }
    if let Ok(text) = value.downcast::<PyString>() {
        return text_value(text);
    }
    Ok(PythonValue::Unencodable(format!(
        "Object of type {} is not JSON serializable",
        type_name(value)?
    )))
}

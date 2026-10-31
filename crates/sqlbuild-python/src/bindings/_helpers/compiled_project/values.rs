//! Python values as the JSON `orjson.dumps(value, option=OPT_SORT_KEYS, default=str)` writes.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString,
    PyStringMethods, PyTuple,
};
use serde_json::{Map, Number, Value};

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::models::NativeRecord;

const DATACLASS_FIELDS: &str = "__dataclass_fields__";
const INSTANCE_DICT: &str = "__dict__";
const ENUM_MODULE: &str = "enum";
const ENUM_TYPE: &str = "Enum";
const DATETIME_MODULE: &str = "datetime";
const DATE_TYPE: &str = "date";
const TIME_TYPE: &str = "time";
const ISOFORMAT: &str = "isoformat";
const ENUM_VALUE: &str = "value";
const PRIVATE_PREFIX: &str = "_";
const ORJSON_WIDE_INTEGER: &str = "Integer exceeds 64-bit range";

/// orjson's value for `value`: enums by value, dataclasses by public attribute, dates in ISO form.
pub(crate) fn plain_value(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    if value.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(flag) = value.downcast::<PyBool>() {
        return Ok(Value::Bool(flag.is_true()));
    }
    if value.is_instance_of::<PyInt>() {
        return integer_value(value);
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(Number::from_f64(value.extract::<f64>()?).map_or(Value::Null, Value::Number));
    }
    if let Ok(text) = value.downcast::<PyString>() {
        return Ok(Value::String(text.to_str()?.to_owned()));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        return value
            .try_iter()?
            .map(|item| plain_value(&item?))
            .collect::<PyResult<Vec<Value>>>()
            .map(Value::Array);
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        return object_value(mapping);
    }
    if let Ok(record) = value.cast::<NativeRecord>() {
        let record = record.get();
        let mut entries = Vec::with_capacity(record.values.len());
        for (field, item) in record.spec.fields.iter().zip(&record.values) {
            if !field.name.starts_with(PRIVATE_PREFIX) {
                entries.push((field.name.to_owned(), plain_value(item.bind(value.py()))?));
            }
        }
        return Ok(sorted_object(entries));
    }
    object_like_value(value)
}

/// Enums, dataclasses and dates as orjson writes them; anything else through `default=str`.
fn object_like_value(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    let py = value.py();
    if value.is_instance(&py.import(ENUM_MODULE)?.getattr(ENUM_TYPE)?)? {
        return plain_value(&native_field(value, ENUM_VALUE)?);
    }
    if value.hasattr(DATACLASS_FIELDS)? && !value.is_instance_of::<pyo3::types::PyType>() {
        return dataclass_value(value);
    }
    let datetime = py.import(DATETIME_MODULE)?;
    if value.is_instance(&native_field(&datetime, DATE_TYPE)?)?
        || value.is_instance(&native_field(&datetime, TIME_TYPE)?)?
    {
        return Ok(Value::String(
            value.call_method0(ISOFORMAT)?.str()?.to_str()?.to_owned(),
        ));
    }
    Ok(Value::String(value.str()?.to_str()?.to_owned()))
}

/// A dataclass's public instance attributes, or its public fields when it has slots.
fn dataclass_value(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    let mut entries: Vec<(String, Value)> = Vec::new();
    if let Ok(attributes) = native_field(value, INSTANCE_DICT) {
        for (key, item) in attributes.downcast::<PyDict>()?.iter() {
            let name: String = key.str()?.to_str()?.to_owned();
            if !name.starts_with(PRIVATE_PREFIX) {
                entries.push((name, plain_value(&item)?));
            }
        }
    } else {
        for key in native_field(value, DATACLASS_FIELDS)?.try_iter()? {
            let name: String = key?.str()?.to_str()?.to_owned();
            if !name.starts_with(PRIVATE_PREFIX) {
                let item = native_field(value, name.as_str())?;
                entries.push((name, plain_value(&item)?));
            }
        }
    }
    Ok(sorted_object(entries))
}

/// A Python integer within 64 bits; orjson rejects wider integers with the same message.
fn integer_value(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Ok(signed) = value.extract::<i64>() {
        return Ok(Value::from(signed));
    }
    match value.extract::<u64>() {
        Ok(unsigned) => Ok(Value::from(unsigned)),
        Err(_) => Err(value_error(ORJSON_WIDE_INTEGER)),
    }
}

/// A string-keyed dictionary with its keys sorted, as `OPT_SORT_KEYS` encodes it.
pub(crate) fn object_value(mapping: &Bound<'_, PyDict>) -> PyResult<Value> {
    let mut entries: Vec<(String, Value)> = Vec::new();
    for (key, item) in mapping.iter() {
        let Ok(key) = key.downcast::<PyString>() else {
            return Err(value_error("Dict key must be str"));
        };
        entries.push((key.to_str()?.to_owned(), plain_value(&item)?));
    }
    Ok(sorted_object(entries))
}

fn sorted_object(mut entries: Vec<(String, Value)>) -> Value {
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Value::Object(entries.into_iter().collect::<Map<String, Value>>())
}

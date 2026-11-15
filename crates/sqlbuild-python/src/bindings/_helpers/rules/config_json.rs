//! Native records and plain values as the JSON `json.dumps(asdict(value), default=str)` writes.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString,
    PyStringMethods, PyTuple,
};
use serde_json::{Map, Number, Value};

use sqlbuild_python_base::records::models::NativeRecord;

const DATACLASS_FIELDS: &str = "__dataclass_fields__";

/// `json.loads(json.dumps(asdict(value), default=str))`, or `None` when only Python spells it.
pub(crate) fn asdict_json(value: &Bound<'_, PyAny>) -> PyResult<Option<Value>> {
    json_value(value, true)
}

/// `value` as `json.loads(json.dumps(value, default=str))` reads it back, or `None` as above.
pub(crate) fn dumped_json(value: &Bound<'_, PyAny>) -> PyResult<Option<Value>> {
    json_value(value, false)
}

/// `None` for non-finite or wide numbers, non-text keys and Python dataclasses.
fn json_value(value: &Bound<'_, PyAny>, records: bool) -> PyResult<Option<Value>> {
    if value.is_none() {
        return Ok(Some(Value::Null));
    }
    if let Ok(flag) = value.downcast::<PyBool>() {
        return Ok(Some(Value::Bool(flag.is_true())));
    }
    if value.is_instance_of::<PyInt>() {
        if let Ok(number) = value.extract::<i64>() {
            return Ok(Some(Value::from(number)));
        }
        if let Ok(number) = value.extract::<u64>() {
            return Ok(Some(Value::from(number)));
        }
        return Ok(None);
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(Number::from_f64(value.extract::<f64>()?).map(Value::Number));
    }
    if let Ok(text) = value.downcast::<PyString>() {
        return Ok(Some(Value::String(text.to_str()?.to_owned())));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<Value> = Vec::new();
        for item in value.try_iter()? {
            let Some(item) = json_value(&item?, records)? else {
                return Ok(None);
            };
            items.push(item);
        }
        return Ok(Some(Value::Array(items)));
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        let mut entries: Map<String, Value> = Map::new();
        for (key, item) in mapping.iter() {
            let Ok(key) = key.downcast::<PyString>() else {
                return Ok(None);
            };
            let Some(item) = json_value(&item, records)? else {
                return Ok(None);
            };
            entries.insert(key.to_str()?.to_owned(), item);
        }
        return Ok(Some(Value::Object(entries)));
    }
    if records && let Ok(record) = value.cast::<NativeRecord>() {
        let record: &NativeRecord = record.get();
        let mut entries: Map<String, Value> = Map::new();
        for (field, item) in record.spec.fields.iter().zip(&record.values) {
            let Some(item) = json_value(item.bind(value.py()), records)? else {
                return Ok(None);
            };
            entries.insert(field.name.to_owned(), item);
        }
        return Ok(Some(Value::Object(entries)));
    }
    if records && value.hasattr(DATACLASS_FIELDS)? {
        return Ok(None);
    }
    Ok(Some(Value::String(value.str()?.to_str()?.to_owned())))
}

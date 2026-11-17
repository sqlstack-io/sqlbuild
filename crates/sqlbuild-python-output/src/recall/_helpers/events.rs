//! Lifecycle events as the JSON object `lifecycle_event_to_json` encodes, built natively.

use crate::recall::main::escaped_text::escaped_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyMapping, PyString,
    PyTuple,
};
use serde_json::{Map, Number, Value};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::recall::constants::EVENT_OPTIONAL_TEXT_FIELDS;
use crate::recall::constants::EVENT_TEXT_FIELDS;
use crate::recall::constants::INVOCATION_METADATA_SCHEMA_VERSION;
use crate::recall::constants::TIMESTAMP_PARTS;
use crate::recall::constants::UTC_ZONE_NAME;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) fn text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    escaped_text(&value.str()?).map(Some)
}

fn json_items(dict: &Bound<'_, PyDict>) -> PyResult<Option<Value>> {
    let mut entries: Map<String, Value> = Map::new();
    for (key, item) in dict.iter() {
        let Ok(key) = key.downcast::<PyString>() else {
            return Ok(None);
        };
        let Some(item) = json_value(&item)? else {
            return Ok(None);
        };
        entries.insert(escaped_text(key)?, item);
    }
    Ok(Some(Value::Object(entries)))
}

/// A lifecycle JSON value as `lifecycle_event_to_json` encodes it; `None` for anything else.
fn json_value(value: &Bound<'_, PyAny>) -> PyResult<Option<Value>> {
    if value.is_none() {
        return Ok(Some(Value::Null));
    }
    if let Ok(flag) = value.downcast::<PyBool>() {
        return Ok(Some(Value::Bool(flag.is_true())));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(value
            .extract::<i64>()
            .map_or(None, |number| Some(number.into())));
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(Number::from_f64(value.extract::<f64>()?).map(Value::Number));
    }
    if let Ok(text) = value.downcast::<PyString>() {
        return Ok(Some(Value::String(escaped_text(text)?)));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<Value> = Vec::new();
        for item in value.try_iter()? {
            let Some(item) = json_value(&item?)? else {
                return Ok(None);
            };
            items.push(item);
        }
        return Ok(Some(Value::Array(items)));
    }
    if let Ok(dict) = value.downcast::<PyDict>() {
        return json_items(dict);
    }
    if let Ok(mapping) = value.downcast::<PyMapping>() {
        let dict: Bound<'_, PyDict> = PyDict::new(value.py());
        dict.update(mapping)?;
        return json_items(&dict);
    }
    Ok(None)
}

/// `occurred_at` as microsecond ISO-8601 with `Z`, when it is a UTC datetime.
fn utc_timestamp(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let zone: Bound<'_, PyAny> = native_field(value, "tzinfo")?;
    if text(&zone)?.as_deref() != Some(UTC_ZONE_NAME) {
        return Ok(None);
    }
    let mut parts: [i64; 7] = [0; 7];
    for (part, name) in parts.iter_mut().zip(TIMESTAMP_PARTS) {
        *part = native_field(value, name)?.extract()?;
    }
    let [year, month, day, hour, minute, second, microsecond] = parts;
    Ok(Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{microsecond:06}Z"
    )))
}

/// The canonical JSON object of a known lifecycle event; `None` when only Python can encode it.
pub(crate) fn event_object(event: &Bound<'_, PyAny>) -> PyResult<Option<Map<String, Value>>> {
    if !event.is_instance(&host_entry(event.py(), ExecutorHost::LifecycleEvent)?)? {
        return Ok(None);
    }
    let mut object: Map<String, Value> = Map::new();
    for name in EVENT_TEXT_FIELDS {
        object.insert(name.to_owned(), text(&native_field(event, name)?)?.into());
    }
    for name in EVENT_OPTIONAL_TEXT_FIELDS {
        object.insert(name.to_owned(), text(&native_field(event, name)?)?.into());
    }
    let schema_version: i64 = native_field(event, "schema_version")?.extract()?;
    object.insert("schema_version".to_owned(), schema_version.into());
    let Some(occurred_at) = utc_timestamp(&native_field(event, "occurred_at")?)? else {
        return Ok(None);
    };
    object.insert("occurred_at".to_owned(), occurred_at.into());
    let Some(payload) = json_value(&native_field(event, "payload")?)? else {
        return Ok(None);
    };
    object.insert("payload".to_owned(), payload);
    if schema_version >= INVOCATION_METADATA_SCHEMA_VERSION {
        let sequence: Option<i64> = native_field(event, "invocation_sequence")?.extract()?;
        object.insert("invocation_sequence".to_owned(), sequence.into());
        let Some(context) = json_value(&native_field(event, "external_context")?)? else {
            return Ok(None);
        };
        object.insert("external_context".to_owned(), context);
    }
    Ok(Some(object))
}

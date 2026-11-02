//! Python-compatible canonical JSON for planner fingerprint metadata.

use crate::json::main::dumps::dumps;
use crate::json::models::{JsonDialect, JsonInteger, JsonValue, StdlibJsonOptions};
use serde_json::{Map, Value};

/// `json.dumps(object, sort_keys=True, separators=(",", ":"), default=str)`.
pub fn canonical_text(object: Map<String, Value>) -> String {
    let dialect: JsonDialect = JsonDialect::Stdlib(StdlibJsonOptions {
        sort_keys: true,
        ..StdlibJsonOptions::new(None).with_separators(",", ":")
    });
    dumps(&json_value(Value::Object(object)), &dialect).unwrap_or_default()
}

fn json_value(value: Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(flag) => JsonValue::Bool(flag),
        Value::Number(number) => match JsonInteger::parse(&number.to_string()) {
            Some(integer) if !number.is_f64() => JsonValue::Integer(integer),
            _ => JsonValue::Float(number.as_f64().unwrap_or_default()),
        },
        Value::String(text) => JsonValue::String(text),
        Value::Array(items) => JsonValue::Array(items.into_iter().map(json_value).collect()),
        Value::Object(object) => JsonValue::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, json_value(value)))
                .collect(),
        ),
    }
}

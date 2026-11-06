//! Decode fingerprint metadata as Python's JSON object reader does.

use serde_json::{Map, Value};

pub fn decoded_object(
    metadata_json: &str,
) -> Result<Option<Map<String, Value>>, serde_json::Error> {
    Ok(match serde_json::from_str::<Value>(metadata_json)? {
        Value::Object(object) => Some(object),
        _ => None,
    })
}

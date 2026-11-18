//! `set_dependency_metadata`: a Python node's identity metadata with its resolved sets recorded.

use serde_json::{Value, json};

use crate::node_set_dependencies::constants::{ENDS_KEY, SET_DEPENDENCIES_KEY, SET_KEY};

/// `metadata_json` with each `(set label, end node labels)` recorded; a non-object is unchanged.
#[must_use]
pub fn set_dependency_metadata(metadata_json: &str, sets: &[(String, Vec<String>)]) -> String {
    let Ok(Value::Object(mut metadata)) = serde_json::from_str::<Value>(metadata_json) else {
        return metadata_json.to_owned();
    };
    let recorded: Vec<Value> = sets
        .iter()
        .map(|(set, ends)| json!({ SET_KEY: set, ENDS_KEY: ends }))
        .collect();
    metadata.insert(SET_DEPENDENCIES_KEY.to_owned(), Value::Array(recorded));
    Value::Object(metadata).to_string()
}

//! Function hashes and identity payload equality of fingerprint metadata.

use serde_json::Value;

use crate::planner_changes::models::IdentityPayload;
use sqlbuild_core::fingerprint_identity::constants::LOCAL_FUNCTION_HASHES_METADATA_KEY;
use sqlbuild_core::fingerprint_identity::main::{
    decoded_object::decoded_object, python_equal::python_equal,
};

/// Python equality of two identity payloads.
pub(crate) fn payloads_equal(left: &IdentityPayload, right: &IdentityPayload) -> bool {
    match (left, right) {
        (IdentityPayload::Object(left), IdentityPayload::Object(right)) => {
            python_equal(&Value::Object(left.clone()), &Value::Object(right.clone()))
        }
        _ => left == right,
    }
}

/// Directly called functions whose recorded hash changed, sorted.
pub(crate) fn changed_local_function_names(
    metadata_json: &str,
    previous_metadata_json: &str,
) -> Vec<String> {
    let previous: Vec<(String, Value)> = local_function_hashes(previous_metadata_json);
    let mut changed: Vec<String> = Vec::new();
    for (name, hash) in local_function_hashes(metadata_json) {
        let recorded: Option<&Value> = previous
            .iter()
            .find(|(previous_name, _)| *previous_name == name)
            .map(|(_, previous_hash)| previous_hash);
        if recorded.is_some_and(|recorded| !python_equal(recorded, &hash)) {
            changed.push(name);
        }
    }
    changed.sort();
    changed
}

fn local_function_hashes(metadata_json: &str) -> Vec<(String, Value)> {
    match decoded_object(metadata_json) {
        Ok(Some(object)) => match object.get(LOCAL_FUNCTION_HASHES_METADATA_KEY) {
            Some(Value::Object(hashes)) => hashes.clone().into_iter().collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

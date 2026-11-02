//! Read the declared-columns hash a fingerprint recorded.

use serde_json::Value;

use sqlbuild_core::fingerprint_identity::constants::DECLARED_COLUMNS_METADATA_KEY;
use sqlbuild_core::fingerprint_identity::main::decoded_object::decoded_object;

/// The recorded declared-columns hash, when the metadata holds one as a string.
pub fn recorded_declared_columns_hash(metadata_json: Option<&str>) -> Option<String> {
    match decoded_object(metadata_json?) {
        Ok(Some(object)) => match object.get(DECLARED_COLUMNS_METADATA_KEY) {
            Some(Value::String(hash)) => Some(hash.clone()),
            _ => None,
        },
        _ => None,
    }
}

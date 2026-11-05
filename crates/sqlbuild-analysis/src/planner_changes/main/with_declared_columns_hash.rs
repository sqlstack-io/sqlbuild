//! Record the declared-columns hash in fingerprint metadata.

use serde_json::Value;

use sqlbuild_core::fingerprint_identity::constants::DECLARED_COLUMNS_METADATA_KEY;
use sqlbuild_core::fingerprint_identity::main::{
    canonical_text::canonical_text, decoded_object::decoded_object,
};

/// Canonical metadata JSON that also records `declared_columns_hash`; text that is not an object is returned unchanged.
pub fn with_declared_columns_hash(metadata_json: &str, declared_columns_hash: &str) -> String {
    match decoded_object(metadata_json) {
        Ok(Some(mut object)) => {
            object.insert(
                DECLARED_COLUMNS_METADATA_KEY.to_owned(),
                Value::String(declared_columns_hash.to_owned()),
            );
            canonical_text(object)
        }
        _ => metadata_json.to_owned(),
    }
}

//! Drop the declared-columns hash from fingerprint metadata.

use sqlbuild_core::fingerprint_identity::constants::DECLARED_COLUMNS_METADATA_KEY;
use sqlbuild_core::fingerprint_identity::main::{
    canonical_text::canonical_text, decoded_object::decoded_object,
};

/// Canonical metadata JSON without the declared-columns hash, or the text unchanged when it has none.
pub fn without_declared_columns_hash(metadata_json: &str) -> String {
    match decoded_object(metadata_json) {
        Ok(Some(mut object)) if object.contains_key(DECLARED_COLUMNS_METADATA_KEY) => {
            object.shift_remove(DECLARED_COLUMNS_METADATA_KEY);
            canonical_text(object)
        }
        _ => metadata_json.to_owned(),
    }
}

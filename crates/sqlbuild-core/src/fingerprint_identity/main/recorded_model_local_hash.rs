//! Recover model-local identity only from a complete recorded planner payload.

use crate::fingerprint_identity::main::model_local_identity_hash::model_local_identity_hash;
use crate::fingerprint_identity::main::normalize_version_identity_metadata::normalize_version_identity_metadata;
use crate::fingerprint_identity::main::{
    canonical_text::canonical_text, decoded_object::decoded_object,
};

pub fn recorded_model_local_hash(query_hash: &str, metadata_json: &str) -> Option<String> {
    let mut object = match decoded_object(metadata_json) {
        Ok(Some(object)) => object,
        Ok(None) | Err(_) => return None,
    };
    if query_hash.is_empty()
        || !object.get("model_name")?.is_string()
        || !object.get("config")?.is_object()
        || !object.get("execution_signature")?.is_object()
        || !object.get("local_function_hashes")?.is_object()
    {
        return None;
    }
    normalize_version_identity_metadata(&mut object, false);
    Some(model_local_identity_hash(
        query_hash,
        &canonical_text(object),
    ))
}

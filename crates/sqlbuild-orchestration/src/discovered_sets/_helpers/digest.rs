//! The digest that says whether a discover function's result changed.

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::discovered_sets::models::DiscoveredResult;
use crate::snapshots::main::publication::canonical_json_bytes::canonical_json_bytes;
use crate::store::errors::ControlError;

/// The SHA-256 of `result`'s canonical JSON: its items in order, its tags and `max_parallel`.
pub(crate) fn discovered_digest(result: &DiscoveredResult) -> Result<String, ControlError> {
    let value =
        serde_json::to_value(result).map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok(sha256_hex(&canonical_json_bytes(&value)?))
}

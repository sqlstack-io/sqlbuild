//! The canonical JSON bytes a snapshot hashes: object keys sorted at every depth.

use serde_json::Value;

use crate::snapshots::_helpers::content::json_bytes;
use crate::store::errors::ControlError;

/// `value` serialized with its object keys sorted, as snapshot content addresses hash it.
pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, ControlError> {
    json_bytes(value)
}

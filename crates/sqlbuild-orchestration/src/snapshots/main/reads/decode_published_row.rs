//! Decode a row of published definitions, naming the snapshot that published them when they no longer decode.

use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::types::Row;

/// `row` as `T`; when it does not decode but names its `snapshot_id`, `ControlError::UnreadableSnapshot` for that snapshot, so only its own trigger or request fails.
pub(crate) fn decode_published_row<T: DeserializeOwned>(row: Row) -> Result<T, ControlError> {
    let snapshot_id = row
        .get("snapshot_id")
        .cloned()
        .map(serde_json::from_value::<Uuid>);
    match (decode_row::<T>(row), snapshot_id) {
        (Err(ControlError::Invalid(detail)), Some(Ok(snapshot_id))) => {
            Err(ControlError::UnreadableSnapshot {
                snapshot_id,
                run_id: None,
                detail,
            })
        }
        (decoded, _) => decoded,
    }
}

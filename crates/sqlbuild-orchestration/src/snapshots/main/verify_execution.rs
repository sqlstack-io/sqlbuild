//! Refuse execution when a worker's local compilation differs from the pinned publication.

use crate::runs::models::RunExecution;
use crate::snapshots::main::node_hashes::node_hashes;
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

/// Display documents and publication generations are not execution inputs.
pub fn verify_execution(
    input: &SnapshotInput,
    expected: &RunExecution,
) -> Result<(), ControlError> {
    let request = &expected.claim.request;
    if input.location_id != request.location_id || input.target != request.target {
        return Err(ControlError::Invalid(
            "worker location or target differs from the pinned run".into(),
        ));
    }
    if input.image_id != request.image_id {
        return Err(ControlError::Invalid(
            "worker image differs from the pinned run".into(),
        ));
    }
    if input.environment_hash != expected.environment_hash {
        return Err(ControlError::Invalid(
            "worker environment differs from the pinned snapshot".into(),
        ));
    }
    if input.sqb_version != expected.sqb_version {
        return Err(ControlError::Invalid(
            "worker SQLBuild version differs from the pinned snapshot".into(),
        ));
    }
    if input.nodes.len() != expected.node_hashes.len() {
        return Err(ControlError::Invalid(
            "worker node set differs from the pinned snapshot".into(),
        ));
    }
    for (key, hash) in node_hashes(input)? {
        if expected.node_hashes.get(&key) != Some(&hash) {
            return Err(ControlError::Invalid(
                "worker node content differs from the pinned snapshot".into(),
            ));
        }
    }
    Ok(())
}

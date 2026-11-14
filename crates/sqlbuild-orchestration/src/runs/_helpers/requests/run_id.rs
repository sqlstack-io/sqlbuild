//! A run ID a person supplies must not be of the kind the server derives for its own runs.

use uuid::Uuid;

use crate::runs::constants::DERIVED_RUN_ID_VERSION;
use crate::store::errors::ControlError;

/// Refuses a name-based run ID, the kind the server derives for group members and retries.
pub(in crate::runs) fn refuse_derived_run_id(run_id: Uuid) -> Result<(), ControlError> {
    if run_id.get_version_num() != DERIVED_RUN_ID_VERSION {
        return Ok(());
    }
    Err(ControlError::Invalid(format!(
        "run_id {run_id} is a name-based UUID (version 5), which the server reserves for the IDs it \
         derives for group members and retries; send a random (version 4) or time-ordered \
         (version 7) UUID"
    )))
}

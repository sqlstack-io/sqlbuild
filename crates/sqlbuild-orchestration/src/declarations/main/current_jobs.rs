//! The target's current jobs and groups, inside the caller's transaction, for other domains' reads.

use uuid::Uuid;

use crate::declarations::_helpers::reads::jobs_in;
use crate::declarations::models::{JobRecord, ServerPoolCaps};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Each current job and group of `target`, its pools against this server's `caps`.
pub(crate) async fn current_jobs(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    caps: &ServerPoolCaps,
) -> Result<(ControlTransaction, Vec<JobRecord>), ControlError> {
    jobs_in(
        transaction,
        (location_id, target),
        &caps.on((location_id, target)),
    )
    .await
}

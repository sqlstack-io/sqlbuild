//! Recover ownership from active claims, not stale lifecycle events or elapsed heartbeat guesses.

use uuid::Uuid;

use crate::runs::constants::MONITOR_BATCH_SIZE;
use crate::runs::models::RunClaim;
use crate::store::errors::ControlError;
use crate::store::main::read_records::read_records;
use crate::store::models::{ControlStore, Parameter};

/// Keyset pages include claimed-but-unstarted work and never include a neighboring scope.
pub async fn list_run_claims(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    after: Option<Uuid>,
) -> Result<Vec<RunClaim>, ControlError> {
    read_records(
        store,
        &format!(
            "SELECT r.worker_instance AS claim_id, r.request_json AS request \
         FROM sqb_runs r \
         WHERE r.worker_instance IS NOT NULL AND r.location_id=$1 AND r.target=$2 AND ($3 IS NULL OR r.run_id>$3) \
         ORDER BY r.run_id LIMIT {MONITOR_BATCH_SIZE}"
        ),
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::OptionalId(after),
        ],
    )
    .await
}

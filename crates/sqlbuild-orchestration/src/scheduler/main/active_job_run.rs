//! The run a `skip` overlap yields to.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The oldest top-level run of the job that is queued or executing and not being cancelled.
pub(crate) async fn active_job_run(
    transaction: ControlTransaction,
    (location_id, target, job_key): (Uuid, &str, &str),
) -> Result<(ControlTransaction, Option<Uuid>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT r.run_id FROM sqb_runs r WHERE r.location_id=$1 AND r.target=$2 AND r.job_key=$3 \
         AND r.parent_run_id IS NULL AND r.status IN ('queued','launched','running','unresponsive') \
         AND r.cancel_requested_at IS NULL ORDER BY r.queued_at, r.run_id LIMIT 1",
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Text(job_key.into()),
        ],
    )
    .await?;
    let run_id = rows
        .first()
        .and_then(|row| row.get("run_id"))
        .and_then(serde_json::Value::as_str)
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok((transaction, run_id))
}

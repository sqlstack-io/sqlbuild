//! Bounded run metadata for one node; it never includes SQL, sampled rows or captured output.

use uuid::Uuid;

use crate::runs::models::NodeRunRecord;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn node_runs_in(
    transaction: ControlTransaction,
    (location_id, target, node_key): (Uuid, &str, &str),
    limit: u32,
) -> Result<(ControlTransaction, Vec<NodeRunRecord>), ControlError> {
    query_records(transaction,
        "SELECT r.run_id, r.job_key, r.queued_at, n.status, n.started_at, n.ended_at, n.rows_affected, n.error_code, \
         n.skipped_because_of FROM sqb_run_nodes n JOIN sqb_runs r ON r.run_id=n.run_id \
         WHERE r.location_id=$1 AND r.target=$2 AND n.node_key=$3 AND n.kind='node' \
         ORDER BY r.queued_at DESC, r.run_id DESC LIMIT $4",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Text(node_key.into()),
            Parameter::Integer(i64::from(limit.min(1000)))]).await
}

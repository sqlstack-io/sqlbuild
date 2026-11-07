//! Job and trigger reads inside a caller's transaction, so authorization and results share one view.

use uuid::Uuid;

use crate::declarations::models::{JobRecord, StoredJob, TriggerRecord};
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn triggers_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Vec<TriggerRecord>), ControlError> {
    query_records(transaction,
        "SELECT t.trigger_id, t.name, t.job_key, t.definition_json, s.status, s.last_tick_for, s.next_due_at \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         WHERE t.location_id=$1 AND t.target=$2 AND t.removed_in_snapshot_id IS NULL ORDER BY t.name",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await
}

pub(crate) async fn jobs_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Vec<JobRecord>), ControlError> {
    let (transaction, stored): (_, Vec<StoredJob>) = query_records(
        transaction,
        "SELECT job_key, kind, snapshot_id, definition_json FROM sqb_jobs \
         WHERE location_id=$1 AND target=$2 ORDER BY job_key",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    let jobs: Vec<JobRecord> = stored
        .into_iter()
        .map(|job| JobRecord {
            job_key: job.job_key,
            kind: job.kind,
            description: job.definition_json.description,
            selection: job.definition_json.selection,
            concurrency_groups: job.definition_json.concurrency_groups,
            snapshot_id: job.snapshot_id,
        })
        .collect();
    Ok((transaction, jobs))
}

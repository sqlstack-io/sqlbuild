//! Publish definition projections inside the current-snapshot transaction.

use uuid::Uuid;

use crate::declarations::_helpers::trigger_state::trigger_timing;
use crate::declarations::models::{Definitions, PreviousTriggerState};
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn publish_definitions(
    mut transaction: ControlTransaction,
    snapshot: &SnapshotRecord,
    definitions: Definitions,
) -> Result<ControlTransaction, ControlError> {
    for table in ["sqb_job_nodes", "sqb_jobs"] {
        (transaction, _) = query(
            transaction,
            &format!("DELETE FROM {table} WHERE location_id=$1 AND target=$2"),
            vec![
                Parameter::Id(snapshot.location_id),
                Parameter::Text(snapshot.target.clone()),
            ],
        )
        .await?;
    }
    let jobs = definitions
        .jobs
        .into_iter()
        .map(|(name, job)| (name, "job", job))
        .chain(
            definitions
                .groups
                .into_iter()
                .map(|(name, group)| (name, "group", group.job)),
        );
    for (name, kind, job) in jobs {
        let definition =
            serde_json::to_value(&job).map_err(|error| ControlError::Invalid(error.to_string()))?;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_jobs (location_id, target, job_key, snapshot_id, kind, definition_json, overlap_policy) VALUES ($1,$2,$3,$4,$7,$5,$6)",
            vec![Parameter::Id(snapshot.location_id), Parameter::Text(snapshot.target.clone()), Parameter::Text(name.clone()),
                Parameter::Id(snapshot.snapshot_id), Parameter::Json(definition), Parameter::Text(job.overlap.as_str().into()), Parameter::Text(kind.into())]).await?;
        for node in job.selection {
            (transaction, _) = query(transaction,
                "INSERT INTO sqb_job_nodes (location_id, target, job_key, node_key) VALUES ($1,$2,$3,$4)",
                vec![Parameter::Id(snapshot.location_id), Parameter::Text(snapshot.target.clone()), Parameter::Text(name.clone()), Parameter::Text(node)]).await?;
        }
    }
    (transaction, _) = query(transaction,
        "UPDATE sqb_triggers SET removed_in_snapshot_id=$1 WHERE location_id=$2 AND target=$3 AND removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(snapshot.snapshot_id), Parameter::Id(snapshot.location_id), Parameter::Text(snapshot.target.clone())]).await?;
    for (name, trigger) in definitions.triggers {
        let id = Uuid::new_v5(
            &snapshot.location_id,
            &serde_json::to_vec(&(&snapshot.target, "schedule", &name))
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        );
        let definition = serde_json::to_value(&trigger)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_triggers (trigger_id, location_id, target, kind, name, job_key, definition_json, snapshot_id) VALUES ($1,$2,$3,'schedule',$4,$5,$6,$7) \
             ON CONFLICT (trigger_id) DO UPDATE SET job_key=excluded.job_key, definition_json=excluded.definition_json, snapshot_id=excluded.snapshot_id, removed_in_snapshot_id=NULL",
            vec![Parameter::Id(id), Parameter::Id(snapshot.location_id), Parameter::Text(snapshot.target.clone()), Parameter::Text(name),
                Parameter::Text(trigger.job_key.clone()), Parameter::Json(definition), Parameter::Id(snapshot.snapshot_id)]).await?;
        let previous;
        (transaction, previous) = query(transaction, "SELECT started_at, last_tick_for, status_override FROM sqb_trigger_state WHERE trigger_id=$1", vec![Parameter::Id(id)]).await?;
        let state = previous
            .into_iter()
            .next()
            .map(decode_row::<PreviousTriggerState>)
            .transpose()?;
        let started_at = state.as_ref().map_or(
            trigger.starts_at.unwrap_or(snapshot.published_at),
            |state| state.started_at,
        );
        let (status, next) = trigger_timing(
            &trigger,
            started_at,
            state.as_ref().and_then(|state| state.last_tick_for),
            state.as_ref().and_then(|state| state.status_override),
        )?;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_trigger_state (trigger_id, status, started_at, next_due_at, updated_at) VALUES ($1,$2,$3,$4,$5) \
             ON CONFLICT (trigger_id) DO UPDATE SET status=excluded.status, next_due_at=excluded.next_due_at, updated_at=excluded.updated_at",
            vec![Parameter::Id(id), Parameter::Text(status.as_str().into()), Parameter::Time(started_at), Parameter::OptionalTime(next), Parameter::Time(snapshot.published_at)]).await?;
    }
    Ok(transaction)
}

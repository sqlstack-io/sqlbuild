//! Job and trigger reads inside a caller's transaction, so authorization and results share one view.

use uuid::Uuid;

use sqlbuild_core::redaction::main::redact_text::redact_text;

use crate::declarations::models::{
    JobRecord, PersonTriggerChange, StoredJob, TriggerChangeRequest, TriggerRecord,
};
use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
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

/// A trigger's operator changes from the security audit, newest first, with a person actor's name.
pub(crate) async fn trigger_changes_in(
    transaction: ControlTransaction,
    trigger_id: Uuid,
    limit: u32,
) -> Result<(ControlTransaction, Vec<PersonTriggerChange>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT a.scope_json, a.occurred_at, p.name AS actor_name FROM sqb_security_audit a \
         LEFT JOIN sqb_principals p ON 'principal:' || CAST(p.principal_id AS TEXT)=a.actor \
         WHERE a.action='trigger.changed' AND a.scope_json ->> 'trigger_id'=$1 \
         ORDER BY a.occurred_at DESC, a.event_id DESC LIMIT $2",
        vec![
            Parameter::Text(trigger_id.to_string()),
            Parameter::Integer(i64::from(limit.min(1000))),
        ],
    )
    .await?;
    let changes = rows
        .into_iter()
        .map(|row| {
            let request: TriggerChangeRequest =
                serde_json::from_value(row["scope_json"]["request"].clone())
                    .map_err(|error| ControlError::Schema(format!("trigger change: {error}")))?;
            let snapshot_id = serde_json::from_value(row["scope_json"]["snapshot_id"].clone())
                .map_err(|error| ControlError::Schema(format!("trigger change: {error}")))?;
            let occurred_at = serde_json::from_value(row["occurred_at"].clone())
                .map_err(|error| ControlError::Schema(format!("trigger change: {error}")))?;
            Ok(PersonTriggerChange {
                event_id: request.event_id,
                change: request.change,
                actor: request.actor,
                actor_name: row["actor_name"].as_str().map(str::to_owned),
                reason: redact_text(&request.reason),
                occurred_at,
                snapshot_id,
            })
        })
        .collect::<Result<_, ControlError>>()?;
    Ok((transaction, changes))
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
            select: job.definition_json.select,
            declared_at: job.definition_json.declared_at,
            owner: job.definition_json.owner,
            overlap: job.definition_json.overlap,
            on_failure: job.definition_json.on_failure,
        })
        .collect();
    Ok((transaction, jobs))
}

/// The newest tick of every published trigger in the target, each found through the trigger-time index.
pub(crate) async fn latest_ticks_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Vec<TickRecord>), ControlError> {
    query_records(transaction,
        "SELECT k.tick_id, k.trigger_id, k.scheduled_for, k.evaluated_at, k.epoch, k.outcome, k.run_id, k.covered_by_run_id, k.active_run_id \
         FROM sqb_triggers t JOIN sqb_ticks k ON k.tick_id=(SELECT x.tick_id FROM sqb_ticks x WHERE x.trigger_id=t.trigger_id \
         ORDER BY x.scheduled_for DESC LIMIT 1) \
         WHERE t.location_id=$1 AND t.target=$2 AND t.removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await
}

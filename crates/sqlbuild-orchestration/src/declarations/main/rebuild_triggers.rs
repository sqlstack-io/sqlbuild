//! Restore recorded trigger images; a missing legacy source aborts the caller's entire repair.

use std::collections::BTreeSet;

use sqlbuild_recall::schema::models::ColumnType;
use uuid::Uuid;

use crate::declarations::_helpers::documents::decode_definitions;
use crate::declarations::models::{TriggerDeclaration, TriggerProjection};
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{Change, ControlTransaction, Parameter};

async fn expected_triggers(
    mut transaction: ControlTransaction,
) -> Result<(ControlTransaction, BTreeSet<Uuid>), ControlError> {
    let mut expected: BTreeSet<Uuid> = BTreeSet::new();
    let mut cursor = None;
    loop {
        let rows;
        (transaction, rows) = query(transaction,
            "SELECT * FROM sqb_snapshots WHERE ($1 IS NULL OR snapshot_id>$1) ORDER BY snapshot_id LIMIT 100",
            vec![Parameter::OptionalId(cursor)]).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            if row["snapshot_schema_version"].as_i64() != Some(1) {
                return Err(ControlError::Invalid(
                    "cannot replay an unsupported snapshot schema".into(),
                ));
            }
            let snapshot: SnapshotRecord = decode_row(row)?;
            let documents;
            (transaction, documents) =
                read_snapshot_documents(transaction, snapshot.snapshot_id, Some(&["triggers"]))
                    .await?;
            let definitions = decode_definitions(&documents)?;
            for (kind, name) in definitions
                .triggers
                .keys()
                .map(|name| ("schedule", name))
                .chain(definitions.sensors.keys().map(|name| ("sensor", name)))
            {
                expected.insert(Uuid::new_v5(
                    &snapshot.location_id,
                    &serde_json::to_vec(&(&snapshot.target, kind, name))
                        .map_err(|error| ControlError::Invalid(error.to_string()))?,
                ));
            }
            cursor = Some(snapshot.snapshot_id);
        }
    }
    Ok((transaction, expected))
}

pub(crate) async fn rebuild_triggers(
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, mut expected) = expected_triggers(transaction).await?;
    let mut cursor = None;
    loop {
        let rows;
        (transaction, rows) = query(transaction,
            "SELECT p.* FROM sqb_trigger_progress p WHERE ($1 IS NULL OR p.trigger_id>$1) \
             AND NOT EXISTS (SELECT 1 FROM sqb_trigger_progress newer WHERE newer.trigger_id=p.trigger_id AND newer.storage_id>p.storage_id) \
             ORDER BY p.trigger_id LIMIT 100", vec![Parameter::OptionalId(cursor)]).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            if row["schema_version"].as_i64() != Some(1) {
                return Err(ControlError::Invalid(
                    "cannot replay an unsupported trigger progress schema".into(),
                ));
            }
            let projection: TriggerProjection =
                serde_json::from_value(row["projection_json"].clone()).map_err(|error| {
                    ControlError::Invalid(format!("invalid trigger progress: {error}"))
                })?;
            let identity = Uuid::new_v5(
                &projection.location_id,
                &serde_json::to_vec(&(&projection.target, &projection.kind, &projection.name))
                    .map_err(|error| ControlError::Invalid(error.to_string()))?,
            );
            if !expected.remove(&identity)
                || identity != projection.trigger_id
                || row["trigger_id"] != serde_json::json!(identity)
                || row["snapshot_id"] != serde_json::json!(projection.snapshot_id)
                || !matches!(
                    (&*projection.kind, &projection.definition_json),
                    ("schedule", TriggerDeclaration::Schedule(_))
                        | ("sensor", TriggerDeclaration::Sensor(_))
                )
            {
                return Err(ControlError::Invalid(
                    "trigger progress does not match retained declarations".into(),
                ));
            }
            cursor = Some(identity);
            transaction = restore_trigger(transaction, projection).await?;
        }
    }
    if !expected.is_empty() {
        return Err(ControlError::Invalid(
            "cannot rebuild triggers: retained legacy history lacks semantic progress; no cursor was inferred".into(),
        ));
    }
    for table in ["sqb_trigger_state", "sqb_triggers"] {
        (transaction, _) = query(transaction,
            &format!("DELETE FROM {table} WHERE NOT EXISTS (SELECT 1 FROM sqb_trigger_progress p WHERE p.trigger_id={table}.trigger_id)"),
            vec![]).await?;
    }
    Ok(transaction)
}

async fn restore_trigger(
    transaction: ControlTransaction,
    projection: TriggerProjection,
) -> Result<ControlTransaction, ControlError> {
    let definition = serde_json::to_value(&projection.definition_json)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, source) = query(transaction,
        "SELECT snapshot_id FROM sqb_snapshots WHERE snapshot_id=$1 AND location_id=$2 AND target=$3",
        vec![Parameter::Id(projection.snapshot_id), Parameter::Id(projection.location_id), Parameter::Text(projection.target.clone())]).await?;
    if source.is_empty() {
        return Err(ControlError::Invalid(
            "trigger progress has no matching publication".into(),
        ));
    }
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_triggers (trigger_id,location_id,target,kind,name,job_key,definition_json,snapshot_id,removed_in_snapshot_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (trigger_id) DO UPDATE SET location_id=excluded.location_id,target=excluded.target, \
         kind=excluded.kind,name=excluded.name,job_key=excluded.job_key,definition_json=excluded.definition_json, \
         snapshot_id=excluded.snapshot_id,removed_in_snapshot_id=excluded.removed_in_snapshot_id",
        vec![Parameter::Id(projection.trigger_id),Parameter::Id(projection.location_id),Parameter::Text(projection.target),
            Parameter::Text(projection.kind),Parameter::Text(projection.name),Parameter::Text(projection.job_key),Parameter::Json(definition),
            Parameter::Id(projection.snapshot_id),Parameter::OptionalId(projection.removed_in_snapshot_id)]).await?;
    let (mut transaction, _) = query(transaction,
        "INSERT INTO sqb_trigger_state (trigger_id,status,status_override,started_at,cursor_json,last_tick_for,next_due_at,consecutive_failures,updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (trigger_id) DO UPDATE SET status=excluded.status,status_override=excluded.status_override, \
         started_at=excluded.started_at,cursor_json=excluded.cursor_json,last_tick_for=excluded.last_tick_for,next_due_at=excluded.next_due_at, \
         consecutive_failures=excluded.consecutive_failures,updated_at=excluded.updated_at",
        vec![Parameter::Id(projection.trigger_id),Parameter::Text(projection.status.as_str().into()),
            Parameter::OptionalText(projection.status_override.map(|value| value.as_str().into())),Parameter::Time(projection.started_at),
            projection.cursor_json.map_or(Parameter::Null(ColumnType::Json), Parameter::Json),Parameter::OptionalTime(projection.last_tick_for),
            Parameter::OptionalTime(projection.next_due_at),Parameter::Integer(projection.consecutive_failures),Parameter::Time(projection.updated_at)]).await?;
    transaction
        .changes
        .insert(Change::Trigger(projection.trigger_id));
    Ok(transaction)
}

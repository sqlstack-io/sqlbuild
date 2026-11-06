//! Check reads over the audit-result mirror, keyed by binding key on its own index.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query, query_records::query_records};
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_history::models::CheckEvidence;
use crate::warehouse_metadata::models::WarehouseAuditRecord;

#[derive(Deserialize)]
pub(crate) struct AuditRow {
    pub(crate) event_id: Uuid,
    pub(crate) record: WarehouseAuditRecord,
}

/// The newest result of every check in the target.
pub(crate) async fn latest_results(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Vec<AuditRow>), ControlError> {
    query_records(transaction,
        "SELECT a.event_id,a.record_json AS record FROM sqb_warehouse_audit_results a \
         WHERE a.location_id=$1 AND a.target=$2 AND NOT EXISTS (SELECT 1 FROM sqb_warehouse_audit_results b \
           WHERE b.location_id=a.location_id AND b.target=a.target AND b.binding_key=a.binding_key \
           AND (b.observed_at,b.event_id)>(a.observed_at,a.event_id)) \
         ORDER BY a.audit_name,a.binding_key",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await
}

/// One result of one check.
pub(crate) async fn check_result(
    transaction: ControlTransaction,
    (location_id, target, binding_key): (Uuid, &str, &str),
    event_id: Uuid,
) -> Result<(ControlTransaction, Option<AuditRow>), ControlError> {
    let (transaction, mut rows) = query_records(
        transaction,
        "SELECT event_id,record_json AS record FROM sqb_warehouse_audit_results \
         WHERE location_id=$1 AND target=$2 AND binding_key=$3 AND event_id=$4",
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Text(binding_key.into()),
            Parameter::Id(event_id),
        ],
    )
    .await?;
    Ok((transaction, rows.pop()))
}

/// Stored evidence rows as columns in sorted key order and aligned cells.
pub(crate) fn evidence(row: AuditRow) -> Result<CheckEvidence, ControlError> {
    let objects: Vec<serde_json::Map<String, serde_json::Value>> = row
        .record
        .evidence_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| {
            ControlError::Schema("stored check evidence is not an array of objects".into())
        })?
        .unwrap_or_default();
    let columns: Vec<String> = objects
        .iter()
        .flat_map(|object| object.keys().cloned())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    let mut rows: Vec<Vec<serde_json::Value>> = Vec::with_capacity(objects.len());
    for object in &objects {
        let mut cells: Vec<serde_json::Value> = Vec::with_capacity(columns.len());
        for column in &columns {
            cells.push(object.get(column).cloned().unwrap_or_default());
        }
        rows.push(cells);
    }
    Ok(CheckEvidence {
        event_id: row.event_id,
        binding_key: row.record.binding_key,
        columns,
        rows,
        truncated: row.record.evidence_truncated,
    })
}

/// One check's results, newest first.
pub(crate) async fn check_results(
    transaction: ControlTransaction,
    (location_id, target, binding_key): (Uuid, &str, &str),
    limit: u32,
) -> Result<(ControlTransaction, Vec<AuditRow>), ControlError> {
    query_records(transaction,
        "SELECT event_id,record_json AS record FROM sqb_warehouse_audit_results \
         WHERE location_id=$1 AND target=$2 AND binding_key=$3 ORDER BY observed_at DESC,event_id DESC LIMIT $4",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Text(binding_key.into()),
            Parameter::Integer(i64::from(limit.clamp(1, 1000)))]).await
}

/// Authorizes `action` on the row's check, projects the row, and audits the read before commit.
pub(crate) async fn sensitive_check_read<T>(
    (transaction, credentials): (ControlTransaction, (&AuditContext, Uuid)),
    (location_id, target, binding_key): (Uuid, &str, &str),
    (row, action, audit_action): (Option<AuditRow>, Action, &str),
    project: impl FnOnce(AuditRow) -> Result<T, ControlError>,
) -> Result<Option<T>, ControlError> {
    let attached = row
        .as_ref()
        .and_then(|row| row.record.attached_target_name.as_deref());
    let (transaction, scope) =
        check_scope(transaction, (location_id, target, attached), action).await?;
    let (transaction, context) =
        authorize_person(transaction, credentials, &scope, DefaultRole::None).await?;
    let event_id = row.as_ref().map(|row| row.event_id);
    let result = row.map(project).transpose()?;
    let transaction = append_audit(
        transaction,
        &context,
        (Uuid::now_v7(), audit_action),
        (
            json!(scope),
            json!(null),
            json!({"binding_key": binding_key, "event_id": event_id, "found": result.is_some()}),
        ),
    )
    .await?;
    commit(transaction).await?;
    Ok(result)
}

/// The check's own node when the current snapshot has it, otherwise every current node.
async fn check_scope(
    transaction: ControlTransaction,
    (location_id, target, attached): (Uuid, &str, Option<&str>),
    action: Action,
) -> Result<(ControlTransaction, PermissionScope), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    let mut nodes: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
        .collect();
    if let Some(attached) = attached.filter(|attached| nodes.contains(*attached)) {
        nodes = [attached.to_owned()].into();
    }
    Ok((
        transaction,
        PermissionScope {
            action,
            location_id: Some(location_id),
            target: Some(target.into()),
            nodes,
        },
    ))
}

//! Check reads over the audit-result mirror, keyed by binding key on its own index.

use std::collections::BTreeSet;

use serde::Deserialize;
use uuid::Uuid;

use crate::credentials::models::{Action, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::{query::query, query_records::query_records};
use crate::store::models::{ControlTransaction, Parameter};
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

/// The check's own node when the current snapshot has it, otherwise every current node.
pub(crate) async fn check_scope(
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

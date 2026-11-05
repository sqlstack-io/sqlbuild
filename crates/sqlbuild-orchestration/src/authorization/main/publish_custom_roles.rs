//! Each published `roles` document replaces the target's custom roles at once; coverage changes enter the change feed.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::authorization::_helpers::custom_roles::{decode_grants, decode_roles, record_role_change};
use crate::credentials::models::RoleGrant;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

pub(crate) async fn publish_custom_roles(
    mut transaction: ControlTransaction,
    record: &SnapshotRecord,
    document: Option<&Value>,
) -> Result<ControlTransaction, ControlError> {
    let roles = decode_roles(document)?;
    let scope = vec![
        Parameter::Id(record.location_id),
        Parameter::Text(record.target.clone()),
    ];
    let rows;
    (transaction, rows) = query(
        transaction,
        "SELECT role_key,resolved_grants_json FROM sqb_effective_roles WHERE location_id=$1 AND target=$2",
        scope.clone(),
    )
    .await?;
    let mut previous: BTreeMap<String, Vec<RoleGrant>> = BTreeMap::new();
    for row in &rows {
        previous.insert(
            row["role_key"].as_str().unwrap_or_default().into(),
            decode_grants(&row["resolved_grants_json"])?,
        );
    }
    let keys: BTreeSet<&String> = previous.keys().chain(roles.keys()).collect();
    for key in keys {
        let (before, after) = (previous.get(key), roles.get(key));
        if before != after {
            transaction = record_role_change(
                transaction,
                (record.snapshot_id, key),
                (before.map(Vec::as_slice), after.map(Vec::as_slice)),
            )
            .await?;
        }
    }
    (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_effective_roles WHERE location_id=$1 AND target=$2",
        scope,
    )
    .await?;
    for (key, grants) in roles {
        let grants = serde_json::to_string(&grants)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        (transaction, _) = query(
            transaction,
            "INSERT INTO sqb_effective_roles (location_id,target,role_key,resolved_grants_json) VALUES ($1,$2,$3,$4)",
            vec![
                Parameter::Id(record.location_id),
                Parameter::Text(record.target.clone()),
                Parameter::Text(key),
                Parameter::Text(grants),
            ],
        )
        .await?;
    }
    transaction.changes.insert(Change::Credentials);
    Ok(transaction)
}

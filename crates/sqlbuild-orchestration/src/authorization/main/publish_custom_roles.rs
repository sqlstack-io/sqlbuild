//! Each published `roles` document replaces the target's custom roles, which take effect at once.

use serde_json::Value;

use crate::authorization::_helpers::custom_roles::decode_roles;
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
    (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_effective_roles WHERE location_id=$1 AND target=$2",
        vec![
            Parameter::Id(record.location_id),
            Parameter::Text(record.target.clone()),
        ],
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

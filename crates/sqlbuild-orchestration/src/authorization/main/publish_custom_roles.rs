//! Record each published custom role; removed pairs and selector coverage apply now, additions wait.

use serde_json::Value;

use crate::authorization::_helpers::custom_roles::{
    decode_roles, effective_role, narrows, retained_grants, set_effective_role,
};
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
    let scope = (record.location_id, record.target.as_str());
    let rows;
    (transaction, rows) = query(
        transaction,
        "SELECT role_key FROM sqb_effective_roles WHERE location_id=$1 AND target=$2",
        vec![Parameter::Id(scope.0), Parameter::Text(scope.1.into())],
    )
    .await?;
    for key in rows
        .iter()
        .filter_map(|row| row["role_key"].as_str())
        .filter(|key| !roles.contains_key(*key))
    {
        (transaction, _) = query(
            transaction,
            "DELETE FROM sqb_effective_roles WHERE location_id=$1 AND target=$2 AND role_key=$3",
            vec![
                Parameter::Id(scope.0),
                Parameter::Text(scope.1.into()),
                Parameter::Text(key.into()),
            ],
        )
        .await?;
        transaction.changes.insert(Change::Credentials);
    }
    for (key, role) in &roles {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_role_definition_versions (location_id,target,role_key,definition_digest,definition_json,first_seen_snapshot_id,first_seen_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
            vec![Parameter::Id(scope.0), Parameter::Text(scope.1.into()), Parameter::Text(key.clone()),
                Parameter::Text(role.digest.clone()), Parameter::Text(role.definition.to_string()),
                Parameter::Id(record.snapshot_id), Parameter::Time(record.published_at)]).await?;
        let effective;
        (transaction, effective) = effective_role(transaction, scope, key).await?;
        let Some((digest, grants)) = effective else {
            continue;
        };
        let next = retained_grants(&role.grants, &grants);
        let next_digest = if narrows(&role.grants, &grants) {
            role.digest.as_str()
        } else {
            digest.as_str()
        };
        if next != grants || next_digest != digest {
            transaction =
                set_effective_role(transaction, (scope.0, scope.1, key), (next_digest, &next))
                    .await?;
        }
    }
    Ok(transaction)
}

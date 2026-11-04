//! Published custom roles narrow at once; a widened definition waits for an administrator approval.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::models::{Action, PublishedRole, RoleDefinition};
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

const ADMINISTRATIVE: [Action; 5] = [
    Action::LocationManage,
    Action::UserManage,
    Action::RoleAssign,
    Action::TokenManage,
    Action::AuditView,
];

const BUILTIN: [&str; 4] = ["viewer", "operator", "developer", "admin"];

fn action_name(action: Action) -> Result<String, ControlError> {
    match serde_json::to_value(action) {
        Ok(Value::String(name)) => Ok(name),
        _ => Err(ControlError::Invalid("unnamed permission action".into())),
    }
}

pub(crate) fn decode_roles(
    document: Option<&Value>,
) -> Result<BTreeMap<String, PublishedRole>, ControlError> {
    let roles: BTreeMap<String, RoleDefinition> = document.map_or_else(
        || Ok(BTreeMap::new()),
        |document| {
            serde_json::from_value(document.clone())
                .map_err(|error| ControlError::Invalid(format!("roles: {error}")))
        },
    )?;
    let mut published: BTreeMap<String, PublishedRole> = BTreeMap::new();
    for (key, definition) in roles {
        if key.trim().is_empty()
            || BUILTIN.contains(&key.to_lowercase().as_str())
            || definition.description.trim().is_empty()
            || definition
                .actions
                .iter()
                .any(|action| ADMINISTRATIVE.contains(action))
        {
            return Err(ControlError::Invalid(format!(
                "role {key} needs a description, a key that is not a built-in role, and only non-administrative actions"
            )));
        }
        let mut grants: BTreeSet<String> = BTreeSet::new();
        for action in definition.actions {
            grants.insert(action_name(action)?);
        }
        let definition = json!({"description": definition.description, "actions": grants});
        let digest = sha256_hex(definition.to_string().as_bytes());
        published.insert(
            key,
            PublishedRole {
                digest,
                grants,
                definition,
            },
        );
    }
    Ok(published)
}

pub(crate) fn decode_grants(value: &Value) -> Result<BTreeSet<String>, ControlError> {
    serde_json::from_str(value.as_str().unwrap_or("[]"))
        .map_err(|error| ControlError::Schema(format!("effective role grants: {error}")))
}

pub(crate) fn grants_allow(value: &Value, action: Action) -> bool {
    match (decode_grants(value), action_name(action)) {
        (Ok(grants), Ok(name)) => grants.contains(&name),
        _ => false,
    }
}

/// The effective digest and grants of one custom role, if any definition is in effect.
pub(crate) async fn effective_role(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    role_key: &str,
) -> Result<(ControlTransaction, Option<(String, BTreeSet<String>)>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT definition_digest,resolved_grants_json FROM sqb_effective_roles WHERE location_id=$1 AND target=$2 AND role_key=$3",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Text(role_key.into())]).await?;
    let Some(row) = rows.first() else {
        return Ok((transaction, None));
    };
    let grants = decode_grants(&row["resolved_grants_json"])?;
    Ok((
        transaction,
        Some((
            row["definition_digest"].as_str().unwrap_or_default().into(),
            grants,
        )),
    ))
}

/// The definition of one custom role in the location's current snapshot.
pub(crate) async fn current_role(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    role_key: &str,
) -> Result<(ControlTransaction, Option<PublishedRole>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT snapshot_id FROM sqb_current_snapshots WHERE location_id=$1 AND target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    let Some(snapshot_id) = rows
        .first()
        .map(|row| serde_json::from_value::<Uuid>(row["snapshot_id"].clone()))
        .transpose()
        .map_err(|error| ControlError::Schema(error.to_string()))?
    else {
        return Ok((transaction, None));
    };
    let (transaction, documents) =
        read_snapshot_documents(transaction, snapshot_id, Some(&["roles"])).await?;
    Ok((
        transaction,
        decode_roles(documents.get("roles"))?.remove(role_key),
    ))
}

pub(crate) async fn set_effective_role(
    mut transaction: ControlTransaction,
    (location_id, target, role_key): (Uuid, &str, &str),
    (digest, grants): (&str, &BTreeSet<String>),
) -> Result<ControlTransaction, ControlError> {
    let grants =
        serde_json::to_string(grants).map_err(|error| ControlError::Invalid(error.to_string()))?;
    let now = transaction.now;
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_effective_roles (location_id,target,role_key,definition_digest,resolved_grants_json,effective_since) \
         VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (location_id,target,role_key) DO UPDATE SET \
         definition_digest=excluded.definition_digest,resolved_grants_json=excluded.resolved_grants_json,effective_since=excluded.effective_since",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Text(role_key.into()),
            Parameter::Text(digest.into()), Parameter::Text(grants), Parameter::Time(now)]).await?;
    transaction.changes.insert(Change::Credentials);
    Ok(transaction)
}

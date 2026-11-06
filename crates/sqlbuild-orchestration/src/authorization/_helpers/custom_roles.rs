//! Published project roles take effect at once; their node coverage follows each snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::models::{Action, PermissionScope, RoleDefinition, RoleGrant};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

const ADMINISTRATIVE: [Action; 3] = [Action::UserManage, Action::RoleAssign, Action::AuditView];

const BUILTIN: [&str; 4] = ["viewer", "operator", "developer", "admin"];

fn action_name(action: Action) -> Result<String, ControlError> {
    match serde_json::to_value(action) {
        Ok(Value::String(name)) => Ok(name),
        _ => Err(ControlError::Invalid("unnamed permission action".into())),
    }
}

fn valid_grant(grant: &RoleGrant) -> bool {
    let blank_selector = grant
        .nodes
        .iter()
        .any(|selector| selector.trim().is_empty());
    let administrative = grant
        .permissions
        .iter()
        .any(|action| ADMINISTRATIVE.contains(action));
    !grant.nodes.is_empty() && !blank_selector && !grant.permissions.is_empty() && !administrative
}

pub(crate) fn decode_roles(
    document: Option<&Value>,
) -> Result<BTreeMap<String, Vec<RoleGrant>>, ControlError> {
    let roles: BTreeMap<String, RoleDefinition> = document.map_or_else(
        || Ok(BTreeMap::new()),
        |document| {
            serde_json::from_value(document.clone())
                .map_err(|error| ControlError::Invalid(format!("roles: {error}")))
        },
    )?;
    let mut published: BTreeMap<String, Vec<RoleGrant>> = BTreeMap::new();
    for (key, definition) in roles {
        if key.trim().is_empty()
            || BUILTIN.contains(&key.to_lowercase().as_str())
            || definition.description.trim().is_empty()
            || definition.grants.is_empty()
            || !definition.grants.iter().all(valid_grant)
        {
            return Err(ControlError::Invalid(format!(
                "role {key} needs a description, a key that is not a built-in role, and grants of node selectors with only non-administrative permissions"
            )));
        }
        let mut grants: Vec<RoleGrant> = definition.grants;
        grants.sort();
        grants.dedup();
        published.insert(key, grants);
    }
    Ok(published)
}

pub(crate) fn decode_grants(value: &Value) -> Result<Vec<RoleGrant>, ControlError> {
    serde_json::from_str(value.as_str().unwrap_or("[]"))
        .map_err(|error| ControlError::Schema(format!("effective role grants: {error}")))
}

/// Nodes on which these effective grants allow the action; a grant that resolved to none gives nothing.
pub(crate) fn granted_nodes(value: &Value, action: Action) -> BTreeSet<String> {
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    for grant in decode_grants(value).unwrap_or_default() {
        if !grant.permissions.contains(&action) {
            continue;
        }
        for selector in &grant.nodes {
            nodes.extend(
                grant
                    .resolved_nodes
                    .get(selector)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
    }
    nodes
}

/// Custom roles whose effective grants add coverage, and requested nodes none covers; built-ins have no grants.
pub(crate) fn custom_coverage(
    rows: &[&Row],
    scope: &PermissionScope,
) -> (Vec<String>, Vec<String>) {
    let mut covered: BTreeSet<String> = BTreeSet::new();
    let mut granting: Vec<String> = Vec::new();
    for row in rows {
        let role_key = row["role_key"].as_str().unwrap_or_default();
        let nodes = granted_nodes(&row["resolved_grants_json"], scope.action);
        let adds = if scope.nodes.is_empty() {
            !nodes.is_empty() && granting.is_empty()
        } else {
            scope
                .nodes
                .iter()
                .any(|node| nodes.contains(node) && !covered.contains(node))
        };
        if !adds {
            continue;
        }
        if !granting.iter().any(|key| key == role_key) {
            granting.push(role_key.into());
        }
        covered.extend(nodes.into_iter().filter(|node| scope.nodes.contains(node)));
    }
    let missing: Vec<String> = scope.nodes.difference(&covered).cloned().collect();
    (granting, missing)
}

/// The audited reason naming up to twenty missing nodes, and node-free text for the caller.
pub(crate) fn denial_reason(
    action: Action,
    missing: &[String],
) -> Result<(String, String), ControlError> {
    let action = action_name(action)?;
    let shown = format!("missing {action} permission for this target");
    if missing.is_empty() {
        return Ok((
            "missing permission in this scope; all requested nodes denied".into(),
            shown,
        ));
    }
    let listed: Vec<&str> = missing.iter().take(20).map(String::as_str).collect();
    let more = missing.len() - listed.len();
    let suffix = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    Ok((
        format!(
            "missing {action} on nodes {}{suffix}; all requested nodes denied",
            listed.join(", ")
        ),
        shown,
    ))
}

fn grants_digest(grants: Option<&[RoleGrant]>) -> Result<Option<String>, ControlError> {
    let Some(grants) = grants else {
        return Ok(None);
    };
    let text =
        serde_json::to_string(grants).map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok(Some(sha256_hex(text.as_bytes())))
}

/// Append a role's coverage change to the snapshot's change feed, after its node changes.
pub(crate) async fn record_role_change(
    transaction: ControlTransaction,
    (snapshot_id, role_key): (Uuid, &str),
    (before, after): (Option<&[RoleGrant]>, Option<&[RoleGrant]>),
) -> Result<ControlTransaction, ControlError> {
    let kind = match (before, after) {
        (None, _) => "added",
        (Some(_), Some(_)) => "changed",
        (Some(_), None) => "removed",
    };
    let (transaction, rows) = query(
        transaction,
        "SELECT COALESCE(MAX(change_seq) + 1, 0) AS next FROM sqb_snapshot_changes WHERE snapshot_id=$1",
        vec![Parameter::Id(snapshot_id)],
    )
    .await?;
    let sequence = rows
        .first()
        .and_then(|row| row["next"].as_i64())
        .unwrap_or_default();
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_snapshot_changes (snapshot_id, change_seq, change_kind, subject_key, before_hash, after_hash) \
         VALUES ($1,$2,$3,$4,$5,$6)",
        vec![Parameter::Id(snapshot_id), Parameter::Integer(sequence), Parameter::Text(kind.into()),
            Parameter::Text(format!("role:{role_key}")), Parameter::OptionalText(grants_digest(before)?),
            Parameter::OptionalText(grants_digest(after)?)]).await?;
    Ok(transaction)
}

//! Reviewed role selectors and permissions narrow at once and widen only by approval; coverage follows snapshots.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::models::{
    Action, PermissionScope, PublishedRole, RoleDefinition, RoleGrant,
};
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};
use crate::store::types::Row;

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
        let mut selectors: BTreeSet<(Vec<String>, Vec<String>)> = BTreeSet::new();
        for grant in &grants {
            let permissions = grant
                .permissions
                .iter()
                .map(|action| action_name(*action))
                .collect::<Result<Vec<_>, _>>()?;
            selectors.insert((grant.nodes.iter().cloned().collect(), permissions));
        }
        let definition = json!({
            "description": definition.description,
            "grants": selectors.iter().map(|(nodes, permissions)| json!({"nodes": nodes, "permissions": permissions})).collect::<Vec<_>>(),
        });
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

fn pairs(grants: &[RoleGrant]) -> BTreeSet<(&str, Action)> {
    let mut pairs: BTreeSet<(&str, Action)> = BTreeSet::new();
    for grant in grants {
        for selector in &grant.nodes {
            for action in &grant.permissions {
                pairs.insert((selector.as_str(), *action));
            }
        }
    }
    pairs
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

/// A denial names at most twenty uncovered nodes so the audited reason stays bounded.
pub(crate) fn denial_reason(action: Action, missing: &[String]) -> Result<String, ControlError> {
    if missing.is_empty() {
        return Ok("missing permission in this scope; all requested nodes denied".into());
    }
    let listed: Vec<&str> = missing.iter().take(20).map(String::as_str).collect();
    let more = missing.len() - listed.len();
    let suffix = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    Ok(format!(
        "missing {} on nodes {}{suffix}; all requested nodes denied",
        action_name(action)?,
        listed.join(", ")
    ))
}

/// Provably narrowing: every published (selector, permission) pair is already effective.
pub(crate) fn narrows(published: &[RoleGrant], effective: &[RoleGrant]) -> bool {
    pairs(published).is_subset(&pairs(effective))
}

/// Effective pairs the publication still contains, one grant per selector with its resolution now.
pub(crate) fn retained_grants(published: &[RoleGrant], effective: &[RoleGrant]) -> Vec<RoleGrant> {
    let kept: BTreeSet<(&str, Action)> = pairs(published)
        .intersection(&pairs(effective))
        .copied()
        .collect();
    let mut grants: BTreeMap<&str, RoleGrant> = BTreeMap::new();
    for (selector, action) in kept {
        let resolved = published
            .iter()
            .find_map(|grant| grant.resolved_nodes.get(selector))
            .cloned()
            .unwrap_or_default();
        grants
            .entry(selector)
            .or_insert_with(|| RoleGrant {
                nodes: BTreeSet::from([selector.to_owned()]),
                permissions: BTreeSet::new(),
                resolved_nodes: BTreeMap::from([(selector.to_owned(), resolved)]),
            })
            .permissions
            .insert(action);
    }
    grants.into_values().collect()
}

/// The effective digest and grants of one custom role, if any definition is in effect.
pub(crate) async fn effective_role(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    role_key: &str,
) -> Result<(ControlTransaction, Option<(String, Vec<RoleGrant>)>), ControlError> {
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

fn grants_digest(grants: Option<&[RoleGrant]>) -> Result<Option<String>, ControlError> {
    let Some(grants) = grants else {
        return Ok(None);
    };
    let text =
        serde_json::to_string(grants).map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok(Some(sha256_hex(text.as_bytes())))
}

/// Append an effective role's coverage change to the snapshot's change feed, after its node changes.
pub(crate) async fn record_role_change(
    transaction: ControlTransaction,
    (snapshot_id, role_key): (Uuid, &str),
    (before, after): (Option<&[RoleGrant]>, Option<&[RoleGrant]>),
) -> Result<ControlTransaction, ControlError> {
    let kind = if after.is_some() {
        "changed"
    } else {
        "removed"
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

pub(crate) async fn set_effective_role(
    mut transaction: ControlTransaction,
    (location_id, target, role_key): (Uuid, &str, &str),
    (digest, grants): (&str, &[RoleGrant]),
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

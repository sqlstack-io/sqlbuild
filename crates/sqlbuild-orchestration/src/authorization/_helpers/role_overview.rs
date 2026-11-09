//! The facts behind a target's role overview, read in the caller's authorized transaction.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::authorization::_helpers::custom_roles::{decode_grants, decode_roles_descriptions};
use crate::authorization::_helpers::person_grants::builtin_actions;
use crate::authorization::models::{CurrentSnapshotId, RoleFacts, RoleHolderCount};
use crate::credentials::models::{BuiltinRole, RoleGrant, RoleOverview, RoleOverviewGrant};
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

const BUILTIN: [(BuiltinRole, &str); 4] = [
    (BuiltinRole::Viewer, "viewer"),
    (BuiltinRole::Operator, "operator"),
    (BuiltinRole::Developer, "developer"),
    (BuiltinRole::Admin, "admin"),
];

/// Published custom roles, active holders per role key, and the current snapshot's role descriptions.
pub(crate) async fn read_role_facts(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, RoleFacts), ControlError> {
    let scope = vec![Parameter::Id(location_id), Parameter::Text(target.into())];
    let (transaction, custom) = query(
        transaction,
        "SELECT role_key, resolved_grants_json FROM sqb_effective_roles \
         WHERE location_id=$1 AND target=$2 ORDER BY role_key",
        scope.clone(),
    )
    .await?;
    let (transaction, holders) = query(
        transaction,
        "SELECT h.role_key, COUNT(DISTINCT h.principal_id) AS holders FROM (\
            SELECT a.principal_id, a.role_key FROM sqb_role_assignments a \
            WHERE (a.location_id IS NULL OR a.location_id=$1) AND (a.target IS NULL OR a.target=$2) \
            UNION ALL \
            SELECT m.principal_id, g.role_key FROM sqb_group_roles g \
            JOIN sqb_person_groups m ON m.group_name=g.group_name \
            WHERE (g.location_id IS NULL OR g.location_id=$1) AND (g.target IS NULL OR g.target=$2)\
         ) h JOIN sqb_principals p ON p.principal_id=h.principal_id WHERE p.disabled_at IS NULL \
         GROUP BY h.role_key",
        scope.clone(),
    )
    .await?;
    let (transaction, current) = query(
        transaction,
        "SELECT snapshot_id FROM sqb_current_snapshots WHERE location_id=$1 AND target=$2",
        scope,
    )
    .await?;
    let current: Option<CurrentSnapshotId> =
        current.into_iter().next().map(decode_row).transpose()?;
    let (transaction, documents) = match current {
        Some(current) => {
            read_snapshot_documents(transaction, current.snapshot_id, Some(&["roles"])).await?
        }
        None => (transaction, BTreeMap::new()),
    };
    let holders: Vec<RoleHolderCount> = holders
        .into_iter()
        .map(decode_row)
        .collect::<Result<_, _>>()?;
    Ok((
        transaction,
        RoleFacts {
            custom,
            holders: holders
                .into_iter()
                .map(|count| (count.role_key, count.holders))
                .collect(),
            descriptions: decode_roles_descriptions(documents.get("roles"))?,
        },
    ))
}

/// Built-in roles first, then custom roles by key.
pub(crate) fn role_overviews(facts: RoleFacts) -> Result<Vec<RoleOverview>, ControlError> {
    let holders_of = |key: &str| facts.holders.get(key).copied().unwrap_or_default();
    let mut roles: Vec<RoleOverview> = BUILTIN
        .into_iter()
        .map(|(role, key)| RoleOverview {
            key: key.into(),
            builtin: true,
            description: None,
            grants: vec![RoleOverviewGrant {
                nodes: Vec::new(),
                permissions: builtin_actions(role),
                matched: BTreeMap::new(),
            }],
            holders: holders_of(key),
        })
        .collect();
    for row in &facts.custom {
        let key = row["role_key"].as_str().unwrap_or_default().to_owned();
        roles.push(RoleOverview {
            holders: holders_of(&key),
            description: facts.descriptions.get(&key).cloned(),
            builtin: false,
            grants: decode_grants(&row["resolved_grants_json"])?
                .into_iter()
                .map(overview_grant)
                .collect(),
            key,
        });
    }
    Ok(roles)
}

fn overview_grant(grant: RoleGrant) -> RoleOverviewGrant {
    let mut matched: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (selector, nodes) in grant.resolved_nodes {
        matched.insert(selector, nodes.into_iter().collect());
    }
    RoleOverviewGrant {
        nodes: grant.nodes.into_iter().collect(),
        permissions: grant.permissions.into_iter().collect(),
        matched,
    }
}

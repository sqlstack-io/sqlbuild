//! A person's live role rows and the side-effect-free decision every person check shares.

use std::collections::BTreeSet;

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::custom_roles::custom_coverage;
use crate::credentials::models::{Action, BuiltinRole, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

const ACTIONS: [Action; 18] = [
    Action::View,
    Action::ViewSql,
    Action::ViewEvidence,
    Action::JobRun,
    Action::RunCancel,
    Action::RunRerun,
    Action::RunRewriteHistory,
    Action::RunPrioritise,
    Action::RunWarehouse,
    Action::RunVars,
    Action::RunLoad,
    Action::TriggerToggle,
    Action::SensorResetCursor,
    Action::RunAdhoc,
    Action::ConnectionTest,
    Action::UserManage,
    Action::RoleAssign,
    Action::AuditView,
];

/// How a scope is decided: the granting role label, or the nodes no role covers.
pub(crate) enum GrantDecision {
    Granted(String),
    Denied(Vec<String>),
}

fn role_allows(role: BuiltinRole, action: Action) -> bool {
    use Action::{
        ConnectionTest, JobRun, RunAdhoc, RunCancel, RunRerun, TriggerToggle, View, ViewSql,
    };
    match role {
        BuiltinRole::Admin => true,
        BuiltinRole::Viewer => action == View,
        BuiltinRole::Operator => {
            matches!(action, View | JobRun | RunCancel | RunRerun | TriggerToggle)
        }
        BuiltinRole::Developer => matches!(
            action,
            View | JobRun
                | RunCancel
                | RunRerun
                | TriggerToggle
                | ViewSql
                | RunAdhoc
                | ConnectionTest
        ),
    }
}

/// A built-in role's actions on every node, in vocabulary order.
pub(crate) fn builtin_actions(role: BuiltinRole) -> Vec<Action> {
    ACTIONS
        .into_iter()
        .filter(|action| role_allows(role, *action))
        .collect()
}

/// The assigned and group roles of an active person for `target`; `None` when the person is disabled or unknown.
pub(crate) async fn person_role_rows(
    transaction: ControlTransaction,
    principal_id: Uuid,
    target: Option<String>,
) -> Result<(ControlTransaction, Option<Vec<Row>>), ControlError> {
    let (transaction, people) = query(
        transaction,
        "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    if people.is_empty() {
        return Ok((transaction, None));
    }
    let (transaction, rows) = query(transaction,
        "SELECT r.role_key,r.location_id,r.target,r.group_name,e.resolved_grants_json FROM \
         (SELECT role_key,location_id,target,NULL AS group_name FROM sqb_role_assignments WHERE principal_id=$1 \
          UNION ALL SELECT g.role_key,g.location_id,g.target,g.group_name FROM sqb_person_groups p \
          JOIN sqb_group_roles g ON g.group_name=p.group_name WHERE p.principal_id=$1) r \
         LEFT JOIN sqb_effective_roles e ON e.location_id=r.location_id AND e.role_key=r.role_key AND e.target=$2 \
         ORDER BY r.role_key,r.group_name",
        vec![Parameter::Id(principal_id), Parameter::OptionalText(target)]).await?;
    Ok((transaction, Some(rows)))
}

/// Decide `scope` from role rows loaded for its target: a built-in role, then custom coverage, then the default role.
pub(crate) fn decide_grant(
    rows: &[Row],
    scope: &PermissionScope,
    default_role: DefaultRole,
) -> GrantDecision {
    let rows: Vec<&Row> = rows
        .iter()
        .filter(|row| {
            (row["location_id"].is_null() || row["location_id"] == json!(scope.location_id))
                && (row["target"].is_null() || row["target"] == json!(scope.target))
        })
        .collect();
    let builtin = rows.iter().find(|row| {
        serde_json::from_value::<BuiltinRole>(row["role_key"].clone())
            .is_ok_and(|role| role_allows(role, scope.action))
    });
    if let Some(row) = builtin {
        let role = row["role_key"].as_str().unwrap_or_default();
        return GrantDecision::Granted(match row["group_name"].as_str() {
            Some(group) => format!("group:{group}:{role}"),
            None => role.into(),
        });
    }
    let (granting, missing) = custom_coverage(&rows, scope);
    if !granting.is_empty() && missing.is_empty() {
        return GrantDecision::Granted(granting.join("+"));
    }
    if default_role == DefaultRole::Viewer && role_allows(BuiltinRole::Viewer, scope.action) {
        return GrantDecision::Granted("default:viewer".into());
    }
    GrantDecision::Denied(missing)
}

/// Actions granted on every node, or on at least one node of a non-empty set.
pub(crate) fn allowed(
    rows: &[Row],
    (location_id, target): (Option<Uuid>, Option<String>),
    nodes: &BTreeSet<String>,
    default_role: DefaultRole,
) -> Vec<Action> {
    ACTIONS
        .into_iter()
        .filter(|action| {
            let scope = PermissionScope {
                action: *action,
                location_id,
                target: target.clone(),
                nodes: nodes.clone(),
            };
            match decide_grant(rows, &scope, default_role) {
                GrantDecision::Granted(_) => true,
                GrantDecision::Denied(missing) => missing.len() < nodes.len(),
            }
        })
        .collect()
}

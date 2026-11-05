//! Live identity and the union of role grants over every requested node decide under the store lock.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::custom_roles::{custom_coverage, denial_reason};
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::models::{Action, AuditContext, BuiltinRole, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

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

pub(crate) async fn authorize_person(
    transaction: ControlTransaction,
    (context, principal_id): (&AuditContext, Uuid),
    scope: &PermissionScope,
    default_role: DefaultRole,
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let mut context = AuditContext {
        actor: format!("principal:{principal_id}"),
        granting_role: "none".into(),
        ..context.clone()
    };
    let (transaction, people) = query(
        transaction,
        "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    if people.is_empty() {
        return deny(
            transaction,
            &context,
            json!(scope),
            "authenticated active person required",
        )
        .await;
    }
    let (transaction, rows) = query(transaction,
        "SELECT a.role_key,a.location_id,a.target,e.resolved_grants_json FROM sqb_role_assignments a \
         LEFT JOIN sqb_effective_roles e ON e.location_id=a.location_id AND e.role_key=a.role_key AND e.target=$2 \
         WHERE a.principal_id=$1 ORDER BY a.role_key",
        vec![Parameter::Id(principal_id), Parameter::OptionalText(scope.target.clone())]).await?;
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
        context.granting_role = row["role_key"].as_str().unwrap_or_default().into();
        return Ok((transaction, context));
    }
    let (granting, missing) = custom_coverage(&rows, scope);
    if !granting.is_empty() && missing.is_empty() {
        context.granting_role = granting.join("+");
        return Ok((transaction, context));
    }
    if default_role == DefaultRole::Viewer && role_allows(BuiltinRole::Viewer, scope.action) {
        context.granting_role = "default:viewer".into();
        return Ok((transaction, context));
    }
    let reason = denial_reason(scope.action, &missing)?;
    deny(transaction, &context, json!(scope), &reason).await
}

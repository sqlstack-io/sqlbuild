//! Live identity and additive built-in grants are evaluated under the operation's store lock.

use serde_json::json;

use crate::authorization::_helpers::custom_roles::grants_allow;
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::main::sessions::authenticate_session_in::authenticate_session_in;
use crate::credentials::models::{
    Action, AuditContext, BuiltinRole, DefaultRole, PermissionScope, SessionAccess,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

fn role_allows(role: BuiltinRole, action: Action) -> bool {
    use Action::{
        ConnectionTest, JobRun, PlanPreview, RunAdhoc, RunCancel, RunRerun, TriggerToggle, View,
        ViewSql,
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
                | PlanPreview
        ),
    }
}

pub(crate) async fn authorize_session(
    transaction: ControlTransaction,
    (context, credentials): (&AuditContext, (&str, &str)),
    scope: &PermissionScope,
    default_role: DefaultRole,
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let access = match scope.action {
        Action::View | Action::ViewSql | Action::ViewEvidence | Action::AuditView => {
            SessionAccess::Read
        }
        _ => SessionAccess::Write {
            csrf: credentials.1,
        },
    };
    let (transaction, identity) =
        authenticate_session_in(transaction, credentials.0, access).await?;
    let mut context = AuditContext {
        actor: identity.as_ref().map_or_else(
            || "unauthenticated".into(),
            |identity| format!("principal:{}", identity.principal_id),
        ),
        granting_role: "none".into(),
        ..context.clone()
    };
    let Some(identity) = identity else {
        return deny(
            transaction,
            &context,
            json!(scope),
            "authenticated person session and CSRF required",
        )
        .await;
    };
    let (transaction, rows) = query(transaction,
        "SELECT a.role_key,a.location_id,a.target,e.resolved_grants_json FROM sqb_role_assignments a \
         LEFT JOIN sqb_effective_roles e ON e.location_id=a.location_id AND e.role_key=a.role_key AND e.target=$2 \
         WHERE a.principal_id=$1 AND a.revoked_at IS NULL ORDER BY a.role_key",
        vec![Parameter::Id(identity.principal_id), Parameter::OptionalText(scope.target.clone())]).await?;
    for row in rows {
        let allowed = serde_json::from_value::<BuiltinRole>(row["role_key"].clone()).map_or_else(
            |_| grants_allow(&row["resolved_grants_json"], scope.action),
            |role| role_allows(role, scope.action),
        );
        if allowed
            && (row["location_id"].is_null() || row["location_id"] == json!(scope.location_id))
            && (row["target"].is_null() || row["target"] == json!(scope.target))
        {
            context.granting_role = row["role_key"].as_str().unwrap_or_default().into();
            return Ok((transaction, context));
        }
    }
    if default_role == DefaultRole::Viewer && role_allows(BuiltinRole::Viewer, scope.action) {
        context.granting_role = "default:viewer".into();
        return Ok((transaction, context));
    }
    deny(
        transaction,
        &context,
        json!(scope),
        "missing permission in this scope; all requested nodes denied",
    )
    .await
}

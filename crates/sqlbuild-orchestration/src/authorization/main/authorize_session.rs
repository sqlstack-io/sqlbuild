//! Live identity and additive built-in grants are evaluated under the operation's store lock.

use serde_json::json;

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
        "SELECT role_key,location_id,target FROM sqb_role_assignments WHERE principal_id=$1 AND revoked_at IS NULL ORDER BY role_key",
        vec![Parameter::Id(identity.principal_id)]).await?;
    for row in rows {
        let Ok(role) = serde_json::from_value::<BuiltinRole>(row["role_key"].clone()) else {
            continue;
        };
        if role_allows(role, scope.action)
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

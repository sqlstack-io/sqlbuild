//! Live identity and the union of assigned and group role grants decide every node under lock.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::custom_roles::denial_reason;
use crate::authorization::_helpers::person_grants::{
    GrantDecision, decide_grant, person_role_rows,
};
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::models::{AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

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
    let (transaction, rows) =
        person_role_rows(transaction, principal_id, scope.target.clone()).await?;
    let Some(rows) = rows else {
        return deny(
            transaction,
            &context,
            json!(scope),
            "authenticated active person required",
        )
        .await;
    };
    match decide_grant(&rows, scope, default_role) {
        GrantDecision::Granted(role) => {
            context.granting_role = role;
            Ok((transaction, context))
        }
        GrantDecision::Denied(missing) => {
            let reason = denial_reason(scope.action, &missing)?;
            deny(transaction, &context, json!(scope), &reason).await
        }
    }
}

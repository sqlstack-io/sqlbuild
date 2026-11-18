//! Decide scopes for a person without recording anything, for dry runs that must not audit while people type.

use uuid::Uuid;

use crate::authorization::_helpers::custom_roles::denial_reason;
use crate::authorization::_helpers::person_grants::{
    GrantDecision, decide_grant, person_role_rows,
};
use crate::credentials::models::{DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Each scope's refusal as a launch would show it, `None` where granted; `None` overall for an inactive person.
pub(crate) async fn decide_person_scopes(
    transaction: ControlTransaction,
    principal_id: Uuid,
    (target, scopes): (&str, &[PermissionScope]),
) -> Result<(ControlTransaction, Option<Vec<Option<String>>>), ControlError> {
    let (transaction, rows) =
        person_role_rows(transaction, principal_id, Some(target.into())).await?;
    let Some(rows) = rows else {
        return Ok((transaction, None));
    };
    let mut refusals: Vec<Option<String>> = Vec::new();
    for scope in scopes {
        refusals.push(match decide_grant(&rows, scope, DefaultRole::None) {
            GrantDecision::Granted(_) => None,
            GrantDecision::Denied(missing) => Some(denial_reason(scope.action, &missing)?.1),
        });
    }
    Ok((transaction, Some(refusals)))
}

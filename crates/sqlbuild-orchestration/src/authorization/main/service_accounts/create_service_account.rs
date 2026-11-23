//! A service account is a principal with one built-in role and a key that is returned only once.

use serde_json::json;

use crate::authorization::_helpers::adder_scopes::required_scopes;
use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::_helpers::service_accounts::{
    add_service_principal, audit_change, begin_change, check_name, read_service_account,
};
use crate::credentials::main::service_keys::issue_service_key::issue_service_key;
use crate::credentials::models::{
    AuditContext, IssuedServiceKey, NewServiceAccount, RoleAssignmentRequest, RoleKey,
    ServiceAccountAuthority,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Adds `request`'s account with a new key or the owner's `chosen_key`; a person needs `user.manage` and `role.assign`.
pub async fn create_service_account(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    (request, chosen_key): (&NewServiceAccount, Option<&str>),
) -> Result<IssuedServiceKey, ControlError> {
    let name = request.name.trim();
    check_name(name)?;
    if chosen_key.is_some() && matches!(authority, ServiceAccountAuthority::Person(_)) {
        return Err(ControlError::Invalid(
            "a chosen key can only be imported with the CLI".into(),
        ));
    }
    let scopes = required_scopes(Some(request.role));
    let (transaction, authorized) = begin_change(store, (context, authority), &scopes).await?;
    let (transaction, principal_id) = add_service_principal(
        transaction,
        &authorized,
        (name, request.description.as_deref()),
    )
    .await?;
    let (transaction, issued) =
        issue_service_key(transaction, &authorized, (principal_id, chosen_key)).await?;
    let transaction = audit_change(
        transaction,
        &authorized,
        (principal_id, "service_account.created"),
        (
            json!(null),
            json!({"name": name, "role": request.role, "key_prefix": issued.prefix,
                   "key_origin": issued.origin}),
        ),
    )
    .await?;
    let role = RoleAssignmentRequest {
        principal_id,
        role: RoleKey::Builtin(request.role),
        location_id: None,
        target: None,
        revoke: false,
    };
    assign_role_in(transaction, &authorized, &role).await?;
    Ok(IssuedServiceKey {
        account: read_service_account(store, principal_id).await?,
        key: issued.key,
    })
}

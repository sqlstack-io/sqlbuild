//! A service account is a principal with one built-in role and a key; the key is returned only once.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::adder_scopes::required_scopes;
use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::_helpers::service_accounts::{
    audit_change, begin_change, check_name, insert_key, service_account_summary,
};
use crate::credentials::_helpers::service_keys::{check_imported_key, new_service_key};
use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::models::{
    AuditContext, IssuedServiceKey, NewServiceAccount, RoleAssignmentRequest, RoleKey,
    ServiceAccountAuthority, ServiceKeyOrigin,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Adds `request`'s service account with a new key, or `chosen_key`, which only the CLI's owner may give.
///
/// A person needs `user.manage` and `role.assign` everywhere.
pub async fn create_service_account(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    (request, chosen_key): (&NewServiceAccount, Option<&str>),
) -> Result<IssuedServiceKey, ControlError> {
    let name = request.name.trim();
    check_name(name)?;
    if let Some(key) = chosen_key {
        if matches!(authority, ServiceAccountAuthority::Person(_)) {
            return Err(ControlError::Invalid(
                "a chosen key can only be imported with the CLI".into(),
            ));
        }
        check_imported_key(key)?;
    }
    let description = request
        .description
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let (transaction, authorized) = begin_change(
        store,
        (context, authority),
        &required_scopes(Some(request.role)),
    )
    .await?;
    let (transaction, taken) = query(
        transaction,
        "SELECT principal_id FROM sqb_principals WHERE lower(name)=$1",
        vec![Parameter::Text(name.into())],
    )
    .await?;
    if !taken.is_empty() {
        return Err(ControlError::Invalid(format!(
            "{name} already names a person or service account"
        )));
    }
    let (transaction, principal_id) =
        insert_principal(transaction, &authorized, (Uuid::now_v7(), name)).await?;
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_service_accounts (principal_id, description, created_by) VALUES ($1,$2,$3)",
        vec![
            Parameter::Id(principal_id),
            Parameter::OptionalText(description.map(str::to_owned)),
            Parameter::Text(authorized.actor.clone()),
        ],
    )
    .await?;
    let (key, origin) = match chosen_key {
        Some(key) => (key.to_owned(), ServiceKeyOrigin::Imported),
        None => (new_service_key()?, ServiceKeyOrigin::Issued),
    };
    let (transaction, prefix) =
        insert_key(transaction, &authorized, (principal_id, &key, origin)).await?;
    let transaction = audit_change(
        transaction,
        &authorized,
        (principal_id, "service_account.created"),
        (
            json!(null),
            json!({"name": name, "description": description, "role": request.role,
                   "key_prefix": prefix, "key_origin": origin}),
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
    let (transaction, account) =
        service_account_summary(begin(store, TransactionMode::Read).await?, principal_id).await?;
    commit(transaction).await?;
    Ok(IssuedServiceKey { account, key })
}

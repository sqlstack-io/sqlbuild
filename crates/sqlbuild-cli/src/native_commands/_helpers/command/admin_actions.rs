//! Token and service account commands the control-store owner runs; a new key appears only in the result.

use serde_json::{Value, json};
use sqlbuild_orchestration::authorization::main::service_accounts::{
    create_service_account::create_service_account, find_service_account::find_service_account,
    list_service_accounts::list_service_accounts, rotate_service_key::rotate_service_key,
    set_service_account_disabled::set_service_account_disabled,
};
use sqlbuild_orchestration::credentials::main::tokens::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::tokens::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::main::tokens::rotate_token::rotate_token;
use sqlbuild_orchestration::credentials::models::{
    AuditContext, ServiceAccountAuthority, ServiceAccountSummary, ServiceKeyRotation,
};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::native_commands::models::{ServiceAccountAdmin, TokenAdmin};

/// A location token command, by the control-store owner.
pub(crate) async fn run_token_admin(
    store: &ControlStore,
    context: &AuditContext,
    action: TokenAdmin,
) -> Result<Value, ControlError> {
    match action {
        TokenAdmin::Issue(scope, expires_at) => {
            Ok(json!(issue_token(store, context, scope, expires_at).await?))
        }
        TokenAdmin::Revoke(token_id) => {
            let revoked = revoke_token(store, context, token_id).await?;
            Ok(json!({"token_id": token_id, "revoked": revoked}))
        }
        TokenAdmin::Rotate(token_id) => Ok(json!(rotate_token(store, context, token_id).await?)),
    }
}

/// A service account command, by the control-store owner.
pub(crate) async fn run_service_account_admin(
    store: &ControlStore,
    context: &AuditContext,
    action: ServiceAccountAdmin,
) -> Result<Value, ControlError> {
    let owner = (context, ServiceAccountAuthority::Owner);
    match action {
        ServiceAccountAdmin::Create { request, key } => {
            let issued = create_service_account(store, owner, (&request, key.as_deref())).await?;
            Ok(key_result(
                &issued.account,
                key.is_none().then_some(&issued.key),
            ))
        }
        ServiceAccountAdmin::Rotate {
            name,
            overlap_minutes,
        } => {
            let principal_id = find_service_account(store, &name).await?;
            let rotation = ServiceKeyRotation {
                overlap_minutes: Some(overlap_minutes),
            };
            let issued = rotate_service_key(store, owner, (principal_id, rotation)).await?;
            Ok(key_result(&issued.account, Some(&issued.key)))
        }
        ServiceAccountAdmin::SetDisabled { name, disabled } => {
            let principal_id = find_service_account(store, &name).await?;
            let account =
                set_service_account_disabled(store, owner, (principal_id, disabled)).await?;
            Ok(json!(account))
        }
        ServiceAccountAdmin::List => Ok(json!(list_service_accounts(store, owner).await?)),
    }
}

/// The account with its new key, or without one when the operator imported their own key.
fn key_result(account: &ServiceAccountSummary, key: Option<&String>) -> Value {
    let mut result = json!({
        "principal_id": account.principal_id,
        "name": account.name,
        "role": account.role,
        "key_prefix": account.keys.first().map(|key| &key.prefix),
        "account": account,
    });
    if let Some(key) = key {
        result["key"] = json!(key);
    }
    result
}

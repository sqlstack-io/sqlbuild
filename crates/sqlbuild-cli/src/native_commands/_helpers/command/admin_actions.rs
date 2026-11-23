//! Token and service account commands the control-store owner runs; a new key appears only in the result.

use sqlbuild_orchestration::authorization::main::service_accounts::{
    create_service_account::create_service_account, find_service_account::find_service_account,
    list_service_accounts::list_service_accounts, rotate_service_key::rotate_service_key,
    set_service_account_disabled::set_service_account_disabled,
};
use sqlbuild_orchestration::credentials::main::identities::recorded_name::recorded_name;
use sqlbuild_orchestration::credentials::main::tokens::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::tokens::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::main::tokens::rotate_token::rotate_token;
use sqlbuild_orchestration::credentials::models::{
    AuditContext, CredentialScope, ServiceAccountAuthority, ServiceKeyRotation,
};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::native_commands::models::{AdminDone, ServiceAccountAdmin, TokenAdmin};
use uuid::Uuid;

/// A location token command, by the control-store owner.
pub(crate) async fn run_token_admin(
    store: &ControlStore,
    context: &AuditContext,
    action: TokenAdmin,
) -> Result<AdminDone, ControlError> {
    let location: String = match &action {
        TokenAdmin::Issue(CredentialScope::Location { location_id }, _) => {
            name_or_id(store, *location_id).await?
        }
        _ => String::new(),
    };
    match action {
        TokenAdmin::Issue(scope, expires_at) => Ok(AdminDone::TokenIssued {
            token: issue_token(store, context, scope, expires_at).await?,
            location,
        }),
        TokenAdmin::Revoke(id) => Ok(AdminDone::TokenRevoked {
            id,
            revoked: revoke_token(store, context, id).await?,
        }),
        TokenAdmin::Rotate(replaced) => Ok(AdminDone::TokenRotated {
            replaced,
            token: rotate_token(store, context, replaced).await?,
        }),
    }
}

/// A service account command, by the control-store owner.
pub(crate) async fn run_service_account_admin(
    store: &ControlStore,
    context: &AuditContext,
    action: ServiceAccountAdmin,
) -> Result<AdminDone, ControlError> {
    let owner = (context, ServiceAccountAuthority::Owner);
    match action {
        ServiceAccountAdmin::Create { request, key } => {
            let issued = create_service_account(store, owner, (&request, key.as_deref())).await?;
            Ok(AdminDone::AccountCreated {
                account: issued.account,
                key: key.is_none().then_some(issued.key),
            })
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
            Ok(AdminDone::KeyRotated {
                account: issued.account,
                key: issued.key,
            })
        }
        ServiceAccountAdmin::SetDisabled { name, disabled } => {
            let principal_id = find_service_account(store, &name).await?;
            Ok(AdminDone::AccountSwitched {
                account: set_service_account_disabled(store, owner, (principal_id, disabled))
                    .await?,
                disabled,
            })
        }
        ServiceAccountAdmin::List => Ok(AdminDone::AccountsListed(
            list_service_accounts(store, owner).await?,
        )),
    }
}

/// What `id` names, read before the change so that a failed read can't hide a committed one or its secret.
pub(crate) async fn name_or_id(store: &ControlStore, id: Uuid) -> Result<String, ControlError> {
    Ok(recorded_name(store, id)
        .await?
        .unwrap_or_else(|| id.to_string()))
}

//! Administrators who manage people or give roles see every service account and its working keys.

use crate::authorization::_helpers::service_accounts::{begin_listing, service_account_summaries};
use crate::credentials::models::{AuditContext, ServiceAccountAuthority, ServiceAccountSummary};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

/// Every service account by name; a person needs `user.manage` or `role.assign` everywhere.
pub async fn list_service_accounts(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
) -> Result<Vec<ServiceAccountSummary>, ControlError> {
    let transaction = begin_listing(store, (context, authority)).await?;
    let (transaction, accounts) = service_account_summaries(transaction, None).await?;
    commit(transaction).await?;
    Ok(accounts)
}

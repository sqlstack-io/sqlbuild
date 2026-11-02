//! Person metadata reads share a live permission decision and the operation's writer fence.

use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::models::{AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, ControlTransaction, TransactionMode};

pub(crate) async fn begin_view(
    store: &ControlStore,
    credentials: (&AuditContext, (&str, &str)),
    scope: &PermissionScope,
    default_role: DefaultRole,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = authorize_session(
        begin(store, TransactionMode::Write).await?,
        credentials,
        scope,
        default_role,
    )
    .await?;
    Ok(transaction)
}

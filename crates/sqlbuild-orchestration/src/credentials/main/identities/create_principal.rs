//! Creating a person or service account never implicitly assigns a role.

use uuid::Uuid;

use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::models::{AuditContext, PrincipalKind};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn create_principal(
    store: &ControlStore,
    context: &AuditContext,
    principal: (&str, PrincipalKind),
) -> Result<Uuid, ControlError> {
    let (transaction, id) = insert_principal(
        begin(store, TransactionMode::Write).await?,
        context,
        principal,
    )
    .await?;
    commit(transaction).await?;
    Ok(id)
}

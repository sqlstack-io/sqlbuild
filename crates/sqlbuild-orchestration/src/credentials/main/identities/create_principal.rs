//! Creating a person never implicitly assigns a role.

use uuid::Uuid;

use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

pub async fn create_principal(
    store: &ControlStore,
    context: &AuditContext,
    name: &str,
) -> Result<Uuid, ControlError> {
    write(store, |transaction| {
        insert_principal(transaction, context, name)
    })
    .await
}

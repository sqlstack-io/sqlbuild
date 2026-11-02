//! Live view permission and a bounded namespace page share the same transaction.

use uuid::Uuid;

use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::catalog::read_catalog_page;
use crate::warehouse_metadata::models::{CatalogPage, CatalogQuery};

pub async fn read_person_catalog(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    (location_id, target, options): (Uuid, &str, &CatalogQuery),
    default_role: DefaultRole,
) -> Result<CatalogPage, ControlError> {
    let (transaction, _) = authorize_session(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        &PermissionScope {
            action: Action::View,
            location_id: Some(location_id),
            target: Some(target.into()),
            nodes: Default::default(),
        },
        default_role,
    )
    .await?;
    let (transaction, page) =
        read_catalog_page(transaction, (location_id, target), options).await?;
    commit(transaction).await?;
    Ok(page)
}

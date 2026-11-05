//! Live view permission and a bounded namespace page share the same transaction.

use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::catalog::read_catalog_page;
use crate::warehouse_metadata::models::{CatalogPage, CatalogQuery};

pub async fn read_person_catalog(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, options): (Uuid, &str, &CatalogQuery),
    default_role: DefaultRole,
) -> Result<CatalogPage, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::View, Some((location_id, target))),
        default_role,
    )
    .await?;
    let (transaction, page) =
        read_catalog_page(transaction, (location_id, target), options).await?;
    commit(transaction).await?;
    Ok(page)
}

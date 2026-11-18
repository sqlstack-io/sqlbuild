//! One read of a target in a transaction where the person may list its current publication.

use std::future::Future;

use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, TransactionMode};

/// Authorize the person to list `(location_id, target)`, run `read`, then commit.
pub(crate) async fn listing_read<T, Read>(
    store: &ControlStore,
    (credentials, default_role): ((&AuditContext, Uuid), DefaultRole),
    (location_id, target): (Uuid, &str),
    read: impl FnOnce(ControlTransaction) -> Read,
) -> Result<T, ControlError>
where
    Read: Future<Output = Result<(ControlTransaction, T), ControlError>>,
{
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, value) = read(transaction).await?;
    commit(transaction).await?;
    Ok(value)
}

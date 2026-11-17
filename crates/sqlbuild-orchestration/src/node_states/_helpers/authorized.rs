//! A person reads a view only after the listing authorization, and only a view of the snapshot that authorization covered.

use std::sync::Arc;

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::node_states::_helpers::views::current_view;
use crate::node_states::models::{NodeStateCache, TargetView};
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::TransactionMode;

pub(crate) async fn authorized_view(
    cache: &NodeStateCache,
    credentials: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Arc<TargetView>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(&cache.store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    current_view(
        cache,
        (location_id, target),
        Some(snapshot.map(|snapshot| snapshot.snapshot_id)),
    )
    .await
}

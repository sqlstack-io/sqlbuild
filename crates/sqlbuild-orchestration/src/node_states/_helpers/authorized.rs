//! A person reads a view only after the listing authorization, and only a view of the snapshot that authorization covered.

use std::sync::Arc;

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::node_states::_helpers::views::current_view;
use crate::node_states::models::{NodeStateCache, TargetView};
use crate::snapshots::main::reads::listing_read::listing_read;
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::store::errors::ControlError;

pub(crate) async fn authorized_view(
    cache: &NodeStateCache,
    credentials: (&AuditContext, Uuid),
    scope: (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Arc<TargetView>, ControlError> {
    let snapshot = listing_read(
        &cache.store,
        (credentials, default_role),
        scope,
        |transaction| read_current_record(transaction, scope),
    )
    .await?;
    current_view(
        cache,
        scope,
        Some(snapshot.map(|snapshot| snapshot.snapshot_id)),
    )
    .await
}

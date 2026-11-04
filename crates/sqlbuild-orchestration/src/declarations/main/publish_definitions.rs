//! Publish definition projections and optional audited development defaults atomically.

use std::collections::BTreeSet;

use crate::authorization::main::publish_custom_roles::publish_custom_roles;
use crate::declarations::_helpers::development::{pause_triggers, trigger_ids};
use crate::declarations::_helpers::publication::publish_projections;
use crate::declarations::models::Definitions;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn publish_definitions(
    transaction: ControlTransaction,
    snapshot: &SnapshotRecord,
    mut definitions: Definitions,
    pause_new_triggers: bool,
) -> Result<ControlTransaction, ControlError> {
    let roles = definitions.roles.take();
    let (transaction, previous) = if pause_new_triggers {
        trigger_ids(transaction, (snapshot.location_id, &snapshot.target)).await?
    } else {
        (transaction, BTreeSet::new())
    };
    let transaction = publish_projections(transaction, snapshot, definitions).await?;
    let transaction = publish_custom_roles(transaction, snapshot, roles.as_ref()).await?;
    if pause_new_triggers {
        pause_triggers(
            transaction,
            (snapshot.location_id, &snapshot.target),
            &previous,
            snapshot.snapshot_id,
        )
        .await
    } else {
        Ok(transaction)
    }
}

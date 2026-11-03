//! Repair projections on one serialized cut; no phase commits independently or rewrites facts.

use crate::declarations::main::rebuild_triggers::rebuild_triggers;
use crate::runs::main::replay_projections::replay_run_projections;
use crate::snapshots::main::rebuild_projections::replay_snapshot_projections;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, verify_store::verify_store};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::main::rebuild_catalog::replay_catalog_projection;

/// Direct control-store ownership is required; this is not a remotely granted operation.
pub async fn rebuild_projections(store: &ControlStore) -> Result<(), ControlError> {
    let transaction = verify_store(store, begin(store, TransactionMode::Write).await?).await?;
    let transaction = replay_snapshot_projections(transaction).await?;
    let transaction = rebuild_triggers(transaction).await?;
    let transaction = replay_run_projections(transaction).await?;
    let transaction = replay_catalog_projection(transaction).await?;
    commit(transaction).await
}

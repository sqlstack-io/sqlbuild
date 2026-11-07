//! A person lists the current snapshot's nodes only with view on every one of them, like jobs.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::catalog_nodes::{catalog_node, latest_builds};
use crate::warehouse_metadata::models::CatalogNodeRecord;

pub async fn list_person_catalog_nodes(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<CatalogNodeRecord>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let Some(snapshot) = snapshot else {
        commit(transaction).await?;
        return Ok(Vec::new());
    };
    let (transaction, nodes) = read_snapshot_nodes(transaction, snapshot.snapshot_id, None).await?;
    let (transaction, builds) = latest_builds(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    Ok(nodes
        .iter()
        .map(|(key, node)| catalog_node((key, node), snapshot.snapshot_id, &builds))
        .collect())
}

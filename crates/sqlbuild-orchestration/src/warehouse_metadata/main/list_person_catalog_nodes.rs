//! A person lists the current nodes with view on all of them; builds are batched indexed seeks.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::catalog_nodes::catalog_node;
use crate::warehouse_metadata::_helpers::fingerprint_reads::latest_fingerprints;
use crate::warehouse_metadata::_helpers::node_status::destination;
use crate::warehouse_metadata::models::{CatalogNodeBuild, CatalogNodeRecord, FingerprintQuery};

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
    let mut positions: Vec<usize> = Vec::new();
    let mut queries: Vec<FingerprintQuery> = Vec::new();
    for (position, (_, node)) in nodes.iter().enumerate() {
        if let Some(query) = destination(node) {
            positions.push(position);
            queries.push(query);
        }
    }
    let (transaction, found) =
        latest_fingerprints(transaction, (location_id, target, &queries)).await?;
    let mut builds: Vec<Option<CatalogNodeBuild>> = vec![None; nodes.len()];
    for (position, build) in positions.iter().zip(found) {
        builds[*position] = build.map(|(record, _)| CatalogNodeBuild {
            observed_at: record.observed_at,
            run_id: record.run_id,
            version_hash: record.version_hash,
        });
    }
    let records: Vec<CatalogNodeRecord> = nodes
        .iter()
        .zip(builds)
        .map(|((key, node), build)| catalog_node((key, node), snapshot.snapshot_id, build))
        .collect();
    commit(transaction).await?;
    Ok(records)
}

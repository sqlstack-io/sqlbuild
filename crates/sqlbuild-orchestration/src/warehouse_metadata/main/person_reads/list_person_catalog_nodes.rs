//! A person lists the current nodes with view on all of them; builds are batched indexed seeks, attempts one bounded read.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::catalog_nodes::{
    catalog_node, last_outcome, latest_attempts,
};
use crate::warehouse_metadata::_helpers::fingerprint_reads::latest_fingerprints;
use crate::warehouse_metadata::_helpers::node_status::{definition_evidence, destination};
use crate::warehouse_metadata::models::{
    CatalogNodeBuild, CatalogNodeRecord, DefinitionEvidence, FingerprintQuery,
};

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
    let nodes: Vec<_> = nodes.iter().collect();
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
    let (transaction, mut attempts) = latest_attempts(transaction, (location_id, target)).await?;
    let mut builds: Vec<Option<CatalogNodeBuild>> = vec![None; nodes.len()];
    let mut changed: Vec<bool> = vec![false; nodes.len()];
    for (position, build) in positions.iter().zip(found) {
        changed[*position] = matches!(
            definition_evidence(nodes[*position].1, build.as_ref()),
            DefinitionEvidence::Changed { .. }
        );
        builds[*position] = build.map(|(record, _)| CatalogNodeBuild {
            observed_at: record.observed_at,
            run_id: record.run_id,
            version_hash: record.version_hash,
        });
    }
    let records: Vec<CatalogNodeRecord> = nodes
        .iter()
        .zip(builds.into_iter().zip(changed))
        .map(|((key, node), (build, changed))| {
            let outcome = last_outcome(attempts.remove(key.as_str()), build.as_ref());
            catalog_node((key, node), snapshot.snapshot_id, (build, outcome, changed))
        })
        .collect();
    commit(transaction).await?;
    Ok(records)
}

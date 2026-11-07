//! A person lists the current nodes with view on all of them; each build is one indexed seek.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::catalog_nodes::catalog_node;
use crate::warehouse_metadata::_helpers::fingerprint_reads::latest_fingerprint;
use crate::warehouse_metadata::_helpers::node_status::destination;
use crate::warehouse_metadata::models::{CatalogNodeBuild, CatalogNodeRecord};

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
    let (mut transaction, nodes) =
        read_snapshot_nodes(transaction, snapshot.snapshot_id, None).await?;
    let mut records: Vec<CatalogNodeRecord> = Vec::with_capacity(nodes.len());
    for (key, node) in &nodes {
        let mut last_build: Option<CatalogNodeBuild> = None;
        if let Some(query) = destination(node) {
            let (next, build) =
                latest_fingerprint(transaction, (location_id, target, &query)).await?;
            transaction = next;
            last_build = build.map(|(record, _)| CatalogNodeBuild {
                observed_at: record.observed_at,
                run_id: record.run_id,
                version_hash: record.version_hash,
            });
        }
        records.push(catalog_node((key, node), snapshot.snapshot_id, last_build));
    }
    commit(transaction).await?;
    Ok(records)
}

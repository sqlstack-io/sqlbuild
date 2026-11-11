//! A person reads one node's column lineage with view on every current node, as the catalog listing requires.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::snapshots::main::read_snapshot_keys::read_snapshot_keys;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::catalog_nodes::{
    neighbourhood, node_columns, published_edges, touched_definitions, touched_nodes,
};
use crate::warehouse_metadata::models::ColumnLineageRead;

/// Columns and edges only, never SQL or expressions, which need view.sql.
pub async fn read_person_column_lineage(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target, node_key): (Uuid, &str, &str),
    default_role: DefaultRole,
) -> Result<ColumnLineageRead, ControlError> {
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
        return Ok(ColumnLineageRead {
            snapshot_id: None,
            nodes: Vec::new(),
            edges: Vec::new(),
        });
    };
    let (transaction, current) = read_snapshot_keys(transaction, snapshot.snapshot_id).await?;
    let (transaction, documents) =
        read_snapshot_documents(transaction, snapshot.snapshot_id, Some(&["lineage"])).await?;
    let edges = neighbourhood(
        &published_edges(documents.get("lineage"), &current),
        node_key,
    );
    let (transaction, nodes) = touched_definitions(
        transaction,
        snapshot.snapshot_id,
        &touched_nodes(node_key, &edges),
    )
    .await?;
    commit(transaction).await?;
    Ok(ColumnLineageRead {
        snapshot_id: Some(snapshot.snapshot_id),
        nodes: node_columns(&nodes),
        edges,
    })
}

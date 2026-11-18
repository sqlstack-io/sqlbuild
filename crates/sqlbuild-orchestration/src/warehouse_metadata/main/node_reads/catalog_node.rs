//! A node's catalog record from its snapshot definition, last build and checks.

use uuid::Uuid;

use crate::snapshots::models::SnapshotNode;
use crate::warehouse_metadata::_helpers::catalog_nodes;
use crate::warehouse_metadata::models::{
    CatalogNodeBuild, CatalogNodeChecks, CatalogNodeOutcome, CatalogNodeRecord,
};

/// A node's catalog record from its snapshot definition, last build and checks.
#[inline]
pub(crate) fn catalog_node(
    node: (&str, &SnapshotNode),
    snapshot_id: Uuid,
    builds: (Option<CatalogNodeBuild>, Option<CatalogNodeOutcome>, bool),
    checks: CatalogNodeChecks,
) -> CatalogNodeRecord {
    catalog_nodes::catalog_node(node, snapshot_id, builds, checks)
}

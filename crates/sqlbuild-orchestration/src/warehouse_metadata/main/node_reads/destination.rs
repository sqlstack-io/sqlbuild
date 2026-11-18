//! Where a node's builds are recorded, when it builds a relation.

use crate::snapshots::models::SnapshotNode;
use crate::warehouse_metadata::_helpers::node_status;
use crate::warehouse_metadata::models::FingerprintQuery;

/// Where a node's builds are recorded, when it builds a relation.
#[inline]
pub(crate) fn destination(node: &SnapshotNode) -> Option<FingerprintQuery> {
    node_status::destination(node)
}

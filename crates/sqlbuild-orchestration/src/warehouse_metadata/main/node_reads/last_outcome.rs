//! A node's last outcome: its newest attempt, or its last build when no attempt is newer.

use crate::warehouse_metadata::_helpers::catalog_nodes;
use crate::warehouse_metadata::models::{CatalogNodeBuild, CatalogNodeOutcome};

/// A node's last outcome: its newest attempt, or its last build when no attempt is newer.
#[inline]
pub(crate) fn last_outcome(
    attempt: Option<CatalogNodeOutcome>,
    build: Option<&CatalogNodeBuild>,
) -> Option<CatalogNodeOutcome> {
    catalog_nodes::last_outcome(attempt, build)
}

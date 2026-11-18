//! Whether each node's definition changed since its recorded build, and why.

use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;
use crate::warehouse_metadata::_helpers::node_status::{self, RecordedBuild};
use crate::warehouse_metadata::models::DefinitionEvidence;

/// Whether each node's definition changed since its recorded build, and why.
#[inline]
pub(crate) async fn definition_evidences(
    transaction: ControlTransaction,
    builds: &[(&SnapshotNode, Option<&RecordedBuild>)],
) -> Result<(ControlTransaction, Vec<DefinitionEvidence>), ControlError> {
    node_status::definition_evidences(transaction, builds).await
}

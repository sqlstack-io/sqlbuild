//! Each node's newest run attempt in a target.

use std::collections::HashMap;

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;
use crate::warehouse_metadata::_helpers::catalog_nodes;
use crate::warehouse_metadata::models::CatalogNodeOutcome;

/// Each node's newest run attempt in a target.
#[inline]
pub(crate) async fn latest_attempts(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, HashMap<String, CatalogNodeOutcome>), ControlError> {
    catalog_nodes::latest_attempts(transaction, scope).await
}

//! Open a transaction that ingests one location session's metadata page.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, ControlTransaction};
use crate::warehouse_metadata::_helpers::ingestion;

/// Open a transaction that ingests one page from a location session of a target.
#[inline]
pub(crate) async fn begin_ingestion(
    store: &ControlStore,
    scope: (Uuid, &str),
    session: (Uuid, i64),
) -> Result<ControlTransaction, ControlError> {
    ingestion::begin_ingestion(store, scope, session).await
}

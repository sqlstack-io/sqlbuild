//! Runs that ended, not a stored request, decide when a location synchronizes again.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::begin_ingestion;
use crate::warehouse_metadata::models::{WarehouseSyncDue, WarehouseSyncDueRead};

pub async fn read_sync_due(
    store: &ControlStore,
    scope: (Uuid, &str),
    request: &WarehouseSyncDueRead,
) -> Result<WarehouseSyncDue, ControlError> {
    let transaction =
        begin_ingestion(store, scope, (request.session_id, request.generation)).await?;
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "SELECT MAX(ended_at) AS ended_at FROM sqb_runs WHERE location_id=$1 AND target=$2",
        vec![Parameter::Id(scope.0), Parameter::Text(scope.1.into())],
    )
    .await?;
    let runs_ended_at = serde_json::from_value(rows[0]["ended_at"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    commit(transaction).await?;
    Ok(WarehouseSyncDue { runs_ended_at, now })
}

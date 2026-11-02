//! The durable mirror, not a separately persisted cursor, owns incremental progress.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::Parameter;
use crate::warehouse_metadata::_helpers::ingestion::begin_ingestion;
use crate::warehouse_metadata::models::{WarehouseHistoryKind, WarehouseWatermarkRead};

pub async fn read_watermark(
    store: &crate::store::models::ControlStore,
    scope: (Uuid, &str),
    request: &WarehouseWatermarkRead,
) -> Result<Option<DateTime<Utc>>, ControlError> {
    let transaction =
        begin_ingestion(store, scope, (request.session_id, request.generation)).await?;
    let table = match request.kind {
        WarehouseHistoryKind::Fingerprints => "sqb_warehouse_fingerprints",
        WarehouseHistoryKind::SourceFreshness => "sqb_warehouse_freshness",
    };
    let mut parameters = vec![
        Parameter::Id(scope.0),
        Parameter::Text(scope.1.into()),
        Parameter::Text(request.location.schema.clone()),
    ];
    let database = match &request.location.database {
        Some(database) => {
            parameters.push(Parameter::Text(database.clone()));
            "state_database=$4"
        }
        None => "state_database IS NULL",
    };
    let (transaction, rows) = query(transaction,
        &format!("SELECT MAX(observed_at) AS watermark FROM {table} WHERE location_id=$1 AND target=$2 AND state_schema=$3 \
            AND {database}"), parameters).await?;
    let watermark = serde_json::from_value(rows[0]["watermark"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    commit(transaction).await?;
    Ok(watermark)
}

//! A plan from synced state reads the target's mirrored catalog and when each relation was seen.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::warehouse_metadata::constants::SYNCED_CATALOG_SQL;
use crate::warehouse_metadata::models::{SyncedCatalogRelation, WarehouseCatalogRecord};

/// Every mirrored relation of the location's target, current as of its last scan's start (no later than its receipt) or later.
pub async fn read_synced_catalog(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
) -> Result<Vec<SyncedCatalogRelation>, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        SYNCED_CATALOG_SQL,
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    commit(transaction).await?;
    rows.into_iter()
        .map(|mut row| {
            let record: WarehouseCatalogRecord =
                serde_json::from_value(row.remove("record_json").unwrap_or_default()).map_err(
                    |error| ControlError::Invalid(format!("invalid mirrored relation: {error}")),
                )?;
            let observed_at: DateTime<Utc> = serde_json::from_value(
                row.remove("observed_at").unwrap_or_default(),
            )
            .map_err(|error| {
                ControlError::Invalid(format!("invalid mirrored observation time: {error}"))
            })?;
            let scan_id: Option<Uuid> = serde_json::from_value(
                row.remove("scan_id").unwrap_or_default(),
            )
            .map_err(|error| ControlError::Invalid(format!("invalid mirrored scan: {error}")))?;
            let received_at: DateTime<Utc> = serde_json::from_value(
                row.remove("received_at").unwrap_or_default(),
            )
            .map_err(|error| {
                ControlError::Invalid(format!("invalid mirrored receipt time: {error}"))
            })?;
            let observed_at = scan_started(scan_id)?.map_or(observed_at, |started| {
                started.min(received_at).max(observed_at)
            });
            Ok(SyncedCatalogRelation {
                record,
                observed_at,
            })
        })
        .collect()
}

/// When the scan that last delivered a record started: a later page of it re-read the record unchanged.
fn scan_started(scan_id: Option<Uuid>) -> Result<Option<DateTime<Utc>>, ControlError> {
    let Some((seconds, nanos)) = scan_id
        .and_then(|scan_id| scan_id.get_timestamp())
        .map(|started| started.to_unix())
    else {
        return Ok(None);
    };
    let seconds: i64 = i64::try_from(seconds)
        .map_err(|error| ControlError::Invalid(format!("invalid mirrored scan time: {error}")))?;
    Ok(DateTime::from_timestamp(seconds, nanos))
}

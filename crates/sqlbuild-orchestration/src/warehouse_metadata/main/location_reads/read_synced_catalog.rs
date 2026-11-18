//! A plan from synced state reads the target's mirrored catalog and when each relation was seen.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::warehouse_metadata::constants::SYNCED_CATALOG_SQL;
use crate::warehouse_metadata::models::{SyncedCatalogRelation, WarehouseCatalogRecord};

/// Every mirrored relation of the location's target, with the time its catalog scan observed it.
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
            let observed_at = serde_json::from_value(row.remove("observed_at").unwrap_or_default())
                .map_err(|error| {
                    ControlError::Invalid(format!("invalid mirrored observation time: {error}"))
                })?;
            Ok(SyncedCatalogRelation {
                record,
                observed_at,
            })
        })
        .collect()
}

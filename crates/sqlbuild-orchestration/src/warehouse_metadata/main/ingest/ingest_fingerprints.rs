//! Import bounded fingerprint history without conflating semantic and exact-content hashes.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::fingerprints::{insert_fingerprint, validate_batch};
use crate::warehouse_metadata::_helpers::ingestion::{
    batch_records, begin_ingestion, commit_ingestion, next_ingestion_chunk,
};
use crate::warehouse_metadata::constants::INGESTION_CHUNK_RECORDS;
use crate::warehouse_metadata::models::{
    MetadataBatch, WarehouseFingerprintRecord, WarehouseSyncResponse,
};

pub async fn ingest_fingerprints(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseFingerprintRecord> = batch_records(batch)?;
    validate_batch(batch, &records)?;
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await?;
    let mut applied = false;
    let mut keys: Vec<Uuid> = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if index > 0 && index % INGESTION_CHUNK_RECORDS == 0 {
            transaction =
                next_ingestion_chunk(transaction, (store, location_id, target), batch, applied)
                    .await?;
        }
        let (next, event_id, inserted) =
            insert_fingerprint(transaction, (location_id, target), record).await?;
        transaction = next;
        keys.push(event_id);
        applied |= inserted;
    }
    commit_ingestion(
        transaction,
        ("sqb_warehouse_fingerprints", location_id, target),
        (batch.scan.as_ref(), &keys),
        applied,
    )
    .await
}

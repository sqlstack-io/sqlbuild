//! Import bounded fingerprint history without conflating semantic and exact-content hashes.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{compress::compress, query::query, store_source_text::store_source_text};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::fingerprints::{source_event_id, validate_batch};
use crate::warehouse_metadata::_helpers::ingestion::{
    batch_records, begin_ingestion, commit_ingestion, scoped_event_id,
};
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
    for record in &records {
        let source_event_id = source_event_id(record);
        let event_id = scoped_event_id(
            (location_id, target),
            (
                record.state_database.as_deref(),
                &record.state_schema,
                "_sqlbuild_fingerprints",
            ),
            &source_event_id,
        )?;
        keys.push(event_id);
        let existing;
        (transaction, existing) = query(
            transaction,
            "SELECT event_id FROM sqb_warehouse_fingerprints WHERE event_id=$1",
            vec![Parameter::Id(event_id)],
        )
        .await?;
        if !existing.is_empty() {
            continue;
        }
        let definition_text_hash = sha256_hex(record.definition.as_bytes());
        let mut record_json = json!(record);
        if let serde_json::Value::Object(object) = &mut record_json {
            object.remove("definition");
        }
        let received_at = transaction.now;
        transaction = store_source_text(
            transaction,
            (
                definition_text_hash.clone(),
                compress(record.definition.as_bytes())?,
            ),
        )
        .await?;
        let inserted;
        (transaction, inserted) = query(transaction,
        "INSERT INTO sqb_warehouse_fingerprints (event_id,location_id,target,source_event_id,observed_at,definition_text_hash,record_json,received_at,state_database,state_schema,node_type,node_name,target_database,target_schema,target_name) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) ON CONFLICT (event_id) DO NOTHING RETURNING event_id",
        vec![Parameter::Id(event_id), Parameter::Id(location_id), Parameter::Text(target.into()),
            Parameter::Text(source_event_id), Parameter::Time(record.ts), Parameter::Text(definition_text_hash),
            Parameter::Json(record_json), Parameter::Time(received_at), Parameter::OptionalText(record.state_database.clone()), Parameter::Text(record.state_schema.clone()),
            Parameter::Text(record.node_type.clone()), Parameter::Text(record.node_name.clone()), Parameter::OptionalText(record.target_database.clone()),
            Parameter::OptionalText(record.target_schema.clone()), Parameter::OptionalText(record.target_name.clone())]).await?;
        applied |= !inserted.is_empty();
    }
    commit_ingestion(
        transaction,
        ("sqb_warehouse_fingerprints", location_id, target),
        (batch.scan.as_ref(), &keys),
        applied,
    )
    .await
}

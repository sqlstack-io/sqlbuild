//! Import bounded fingerprint history without conflating semantic and exact-content hashes.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{
    commit::commit, compress::compress, query::query, store_source_text::store_source_text,
};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::fingerprints::{source_event_id, validate_batch};
use crate::warehouse_metadata::_helpers::ingestion::{begin_ingestion, scoped_event_id};
use crate::warehouse_metadata::models::FingerprintBatch;

pub async fn ingest_fingerprints(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    batch: &FingerprintBatch,
) -> Result<bool, ControlError> {
    validate_batch(batch)?;
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await?;
    let mut applied = false;
    for record in &batch.records {
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
            "INSERT INTO sqb_warehouse_fingerprints (event_id,location_id,target,source_event_id,observed_at,definition_text_hash,record_json,received_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (event_id) DO NOTHING RETURNING event_id",
            vec![Parameter::Id(event_id), Parameter::Id(location_id), Parameter::Text(target.into()),
                Parameter::Text(source_event_id), Parameter::Time(record.ts), Parameter::Text(definition_text_hash),
                Parameter::Json(record_json), Parameter::Time(received_at)]).await?;
        applied |= !inserted.is_empty();
    }
    commit(transaction).await?;
    Ok(applied)
}

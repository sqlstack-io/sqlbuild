//! Fixed legacy fingerprint keys and validation of bounded source history pages.

use serde_json::json;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{compress::compress, query::query, store_source_text::store_source_text};
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{
    outside_scan, scoped_event_id, validate_fields,
};
use crate::warehouse_metadata::models::{MetadataBatch, WarehouseFingerprintRecord};

pub(crate) fn validate_batch(
    batch: &MetadataBatch,
    records: &[WarehouseFingerprintRecord],
) -> Result<(), ControlError> {
    let invalid = records
        .iter()
        .flat_map(|record| {
            [
                &record.state_schema,
                &record.node_type,
                &record.node_name,
                &record.run_id,
            ]
        })
        .any(|value| value.trim().is_empty());
    let invalid_id = records
        .iter()
        .filter_map(|record| record.event_id.as_deref())
        .any(|value| value.trim().is_empty());
    validate_fields(
        batch,
        invalid || invalid_id,
        outside_scan(
            batch,
            records
                .iter()
                .map(|record| (&record.state_database, &record.state_schema)),
        ),
    )
}

/// Hash fixed predecessor fields only when the warehouse has no persisted identity.
pub(crate) fn source_event_id(record: &WarehouseFingerprintRecord) -> String {
    if let Some(event_id) = &record.event_id {
        return event_id.clone();
    }
    sha256_hex(
        json!([
            "sqlbuild.fingerprint.legacy.v1",
            record.node_type,
            record.node_name,
            record.target_database,
            record.target_schema,
            record.target_name,
            record.run_id,
            record.definition_hash,
            record.version_hash,
            record.schema_fingerprint,
            record.definition,
            record.metadata_json,
            record.ts.format("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string(),
        ])
        .to_string()
        .as_bytes(),
    )
}

/// Store one fingerprint and its definition text; its event ID, and whether it was new.
pub(crate) async fn insert_fingerprint(
    mut transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    record: &WarehouseFingerprintRecord,
) -> Result<(ControlTransaction, Uuid, bool), ControlError> {
    let source_event_id = source_event_id(record);
    let event_id = scoped_event_id(
        (location_id, target),
        (
            record.state_database.as_deref(),
            &record.state_schema,
            FINGERPRINT_TABLE_NAME,
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
        return Ok((transaction, event_id, false));
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
    Ok((transaction, event_id, !inserted.is_empty()))
}

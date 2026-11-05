//! Validate source observations and sequence their receipt for durable sensor consumption.

use chrono::SubsecRound;
use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{outside_scan, validate_fields};
use crate::warehouse_metadata::models::{
    MetadataBatch, SourceFreshnessObservation, WarehouseFreshnessRecord,
};

pub(crate) fn validate_batch(
    batch: &MetadataBatch,
    records: &[WarehouseFreshnessRecord],
) -> Result<(), ControlError> {
    let invalid_identity = records
        .iter()
        .flat_map(|record| {
            [
                &record.state_schema,
                &record.source_name,
                &record.run_id,
                &record.strategy,
                &record.value_kind,
                &record.data_version_hash,
            ]
        })
        .any(|value| value.trim().is_empty());
    let invalid_source_id = records
        .iter()
        .filter_map(|record| record.event_id.as_deref())
        .any(|value| value.trim().is_empty());
    validate_fields(
        batch,
        invalid_identity || invalid_source_id,
        outside_scan(
            batch,
            records
                .iter()
                .map(|record| (&record.state_database, &record.state_schema)),
        ),
    )
}

pub(crate) async fn append_observation(
    transaction: ControlTransaction,
    (location_id, target, source_id): (Uuid, &str, Uuid),
    record: &WarehouseFreshnessRecord,
) -> Result<ControlTransaction, ControlError> {
    if record.data_version.is_none() {
        return Ok(transaction);
    }
    let event_id = Uuid::new_v5(&source_id, b"source-freshness-observation");
    let occurred_at = record.observed_at.trunc_subsecs(6);
    let (transaction, existing) = query(
        transaction,
        "SELECT event_id FROM recall_events WHERE occurred_at=$1 AND event_id=$2 LIMIT 1",
        vec![
            Parameter::Time(occurred_at),
            Parameter::Text(event_id.to_string()),
        ],
    )
    .await?;
    if !existing.is_empty() {
        return Ok(transaction);
    }
    let scope = serde_json::to_vec(&(
        target,
        &record.state_database,
        &record.state_schema,
        &record.source_name,
        &record.target_database,
        &record.target_schema,
        &record.target_name,
    ))
    .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let source = Uuid::new_v5(&location_id, &scope);
    let (transaction, appended) = append_control_event(
        transaction,
        ControlEvent {
            event_id,
            invocation_id: format!("warehouse/source-freshness/{source}"),
            event_type: "warehouse_source_freshness",
            producer: "warehouse",
            occurred_at,
            run_id: None,
            payload: json!(SourceFreshnessObservation {
                location_id,
                target: target.into(),
                record: record.clone(),
            }),
        },
    )
    .await?;
    if appended.conflict.is_some() {
        return Err(ControlError::Invalid(
            "source observation conflicts with a durable fact".into(),
        ));
    }
    Ok(transaction)
}

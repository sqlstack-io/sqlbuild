//! Structural facts and their current projection always commit together.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::models::{ControlEvent, ControlTransaction};
use crate::warehouse_metadata::_helpers::ingestion::validate_fields;
use crate::warehouse_metadata::models::CatalogBatch;

pub(crate) fn validate_catalog(batch: &CatalogBatch) -> Result<(), ControlError> {
    let page = &batch.reconciliation;
    let mut names: BTreeSet<&String> = BTreeSet::new();
    validate_fields(
        ("catalog", batch.generation, batch.records.len()),
        batch.records.iter().any(|record| {
            record.name.trim().is_empty()
                || record.relation_type.trim().is_empty()
                || !names.insert(&record.name)
        }),
        page.day.is_some()
            || batch.records.iter().any(|record| {
                record.state_database != page.state_database
                    || record.state_schema != page.state_schema
            }),
    )?;
    for record in &batch.records {
        if record
            .columns
            .iter()
            .any(|column| column.name.trim().is_empty() || column.data_type.trim().is_empty())
        {
            return Err(ControlError::Invalid(
                "catalog columns need nonempty names and types".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) async fn append_catalog_fact(
    transaction: ControlTransaction,
    (scan_id, page, relation_id): (Uuid, u32, Uuid),
    (location_id, target): (Uuid, &str),
    (record, present): (Value, bool),
) -> Result<ControlTransaction, ControlError> {
    let event_id = Uuid::new_v5(
        &scan_id,
        json!((page, relation_id, present)).to_string().as_bytes(),
    );
    let occurred_at = transaction.now;
    let (transaction, outcome) = append_control_event(transaction, ControlEvent {
        event_id,
        invocation_id: format!("warehouse/catalog/{relation_id}"),
        event_type: "warehouse_catalog_changed",
        producer: "warehouse",
        occurred_at,
        run_id: None,
        payload: json!({"location_id":location_id,"target":target,"relation_id":relation_id,"record":record,"present":present}),
    }).await?;
    if outcome.conflict.is_some() {
        return Err(ControlError::Invalid(
            "catalog observation conflicts with a durable fact".into(),
        ));
    }
    Ok(transaction)
}

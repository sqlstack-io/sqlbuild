//! Structural facts and their current projection always commit together.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::validate_fields;
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{
    CatalogBatch, CatalogObservation, CatalogPage, CatalogQuery, WarehouseReconciliationCursor,
};

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

pub(crate) async fn read_catalog_page(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    options: &CatalogQuery,
) -> Result<(ControlTransaction, CatalogPage), ControlError> {
    if options.schema.trim().is_empty()
        || options.after_observed_at.is_some() != options.after_event_id.is_some()
    {
        return Err(ControlError::Invalid(
            "catalog reads require a namespace and both cursor fields or neither".into(),
        ));
    }
    let limit = options
        .limit
        .unwrap_or(100)
        .clamp(1, MAX_HISTORY_RECORDS as u32) as usize;
    let mut parameters = vec![
        Parameter::Id(location_id),
        Parameter::Text(target.into()),
        Parameter::Text(options.schema.clone()),
    ];
    let database = if let Some(database) = &options.database {
        parameters.push(Parameter::Text(database.clone()));
        "state_database=$4"
    } else {
        "state_database IS NULL"
    };
    let mut after = String::new();
    if let (Some(time), Some(id)) = (options.after_observed_at, options.after_event_id) {
        let offset = parameters.len();
        parameters.extend([Parameter::Time(time), Parameter::Id(id)]);
        after = format!(
            " AND (observed_at,event_id)>(${},${})",
            offset + 1,
            offset + 2
        );
    }
    parameters.push(Parameter::Integer(limit as i64));
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT event_id,observed_at,record_json AS record FROM sqb_warehouse_catalog \
            WHERE location_id=$1 AND target=$2 AND state_schema=$3 AND {database}{after} \
            ORDER BY observed_at,event_id LIMIT ${}",
            parameters.len()
        ),
        parameters,
    )
    .await?;
    let more = rows.len() == limit;
    let records: Vec<CatalogObservation> = rows
        .into_iter()
        .map(|row| serde_json::from_value(json!(row)))
        .collect::<Result<_, _>>()
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let next_after =
        more.then(|| records.last())
            .flatten()
            .map(|record| WarehouseReconciliationCursor {
                observed_at: record.observed_at,
                event_id: record.event_id,
            });
    Ok((
        transaction,
        CatalogPage {
            records,
            next_after,
        },
    ))
}

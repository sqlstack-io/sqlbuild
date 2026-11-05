//! The current catalog projection and its paged namespace reads.

use std::collections::BTreeSet;

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{outside_scan, validate_fields};
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{
    CatalogObservation, CatalogPage, CatalogQuery, MetadataBatch, WarehouseCatalogRecord,
    WarehouseMirrorCursor,
};

pub(crate) fn validate_catalog(
    batch: &MetadataBatch,
    records: &[WarehouseCatalogRecord],
) -> Result<(), ControlError> {
    let mut names: BTreeSet<&String> = BTreeSet::new();
    validate_fields(
        batch,
        records.iter().any(|record| {
            record.name.trim().is_empty()
                || record.relation_type.trim().is_empty()
                || !names.insert(&record.name)
        }),
        batch.scan.is_none()
            || outside_scan(
                batch,
                records
                    .iter()
                    .map(|record| (&record.state_database, &record.state_schema)),
            ),
    )?;
    for record in records {
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

pub(crate) async fn project_catalog_record(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    relation_id: Uuid,
    record: &WarehouseCatalogRecord,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_warehouse_catalog (event_id,location_id,target,state_database,state_schema,observed_at,record_json,received_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$6) ON CONFLICT (event_id) DO UPDATE SET \
         observed_at=excluded.observed_at,record_json=excluded.record_json,received_at=excluded.received_at",
        vec![Parameter::Id(relation_id), Parameter::Id(location_id), Parameter::Text(target.into()),
            Parameter::OptionalText(record.state_database.clone()), Parameter::Text(record.state_schema.clone()),
            Parameter::Time(now), Parameter::Json(json!(record))]).await?;
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
    let next_after = more
        .then(|| records.last())
        .flatten()
        .map(|record| WarehouseMirrorCursor {
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

//! Exact relation/time selection never joins definitions or infers data currency.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;
use crate::warehouse_metadata::_helpers::ingestion::scoped_event_id;
use crate::warehouse_metadata::_helpers::mirror_pages::page_tail;
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{
    StatisticsObservation, StatisticsPage, StatisticsQuery, StatisticsRetention,
    WarehouseCatalogRecord, WarehouseMirrorCursor,
};

pub(crate) fn statistics_event_id(
    scope: (Uuid, &str),
    (database, schema, name): (Option<&str>, &str, &str),
    observed_at: DateTime<Utc>,
) -> Result<Uuid, ControlError> {
    scoped_event_id(
        scope,
        (database, schema, "statistics"),
        &json!((name, observed_at.timestamp().div_euclid(3600))).to_string(),
    )
}

/// A partial namespace page never suppresses another relation's missing current-hour sample.
pub(crate) async fn eligible_names(
    mut transaction: ControlTransaction,
    scope: (Uuid, &str),
    rows: Vec<Row>,
) -> Result<(ControlTransaction, Vec<String>), ControlError> {
    let mut candidates: Vec<(Uuid, String)> = Vec::new();
    for row in rows {
        let record: WarehouseCatalogRecord = serde_json::from_value(row["record_json"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let id = statistics_event_id(
            scope,
            (
                record.state_database.as_deref(),
                &record.state_schema,
                &record.name,
            ),
            transaction.now,
        )?;
        candidates.push((id, record.name));
    }
    let mut committed: BTreeSet<Uuid> = BTreeSet::new();
    if !candidates.is_empty() {
        let placeholders = Parameter::placeholders(1..=candidates.len());
        let found;
        (transaction, found) = query(
            transaction,
            &format!(
                "SELECT event_id FROM sqb_warehouse_statistics WHERE event_id IN ({placeholders})"
            ),
            candidates
                .iter()
                .map(|(id, _)| Parameter::Id(*id))
                .collect(),
        )
        .await?;
        for row in found {
            committed.insert(
                serde_json::from_value::<Uuid>(row["event_id"].clone())
                    .map_err(|error| ControlError::Invalid(error.to_string()))?,
            );
        }
    }
    Ok((
        transaction,
        candidates
            .into_iter()
            .filter(|(id, _)| !committed.contains(id))
            .map(|(_, name)| name)
            .collect(),
    ))
}

pub(crate) async fn read_statistics_page(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    (options, retention): (&StatisticsQuery, StatisticsRetention),
) -> Result<(ControlTransaction, StatisticsPage), ControlError> {
    if options.schema.trim().is_empty()
        || options.name.trim().is_empty()
        || options.after_observed_at.is_some() != options.after_event_id.is_some()
    {
        return Err(ControlError::Invalid(
            "statistics reads need an exact relation and both cursor fields or neither".into(),
        ));
    }
    let limit = options
        .limit
        .unwrap_or(MAX_HISTORY_RECORDS as u32)
        .clamp(1, MAX_HISTORY_RECORDS as u32) as usize;
    let cutoff = retention.cutoff(transaction.now);
    let parameters = vec![
        Parameter::Id(location_id),
        Parameter::Text(target.into()),
        Parameter::Text(options.schema.clone()),
        Parameter::Text(options.name.clone()),
        Parameter::Time(cutoff),
    ];
    let (parameters, tail) = page_tail(
        parameters,
        options.database.as_ref(),
        (options.after_observed_at, options.after_event_id),
        limit,
    );
    let (transaction, records) = query_records(transaction,
        &format!("SELECT event_id,record_json AS record FROM sqb_warehouse_statistics \
            WHERE location_id=$1 AND target=$2 AND state_schema=$3 AND relation_name=$4 AND observed_at>=$5{tail}"),
        parameters).await?;
    let records: Vec<StatisticsObservation> = records;
    let next_after = (records.len() == limit)
        .then(|| records.last())
        .flatten()
        .map(|observation| WarehouseMirrorCursor {
            observed_at: observation.record.observed_at,
            event_id: observation.event_id,
        });
    Ok((
        transaction,
        StatisticsPage {
            records,
            next_after,
        },
    ))
}

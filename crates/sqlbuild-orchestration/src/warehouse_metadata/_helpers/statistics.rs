//! Exact relation/time selection never joins definitions or infers data currency.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::store::types::Row;
use crate::warehouse_metadata::_helpers::ingestion::scoped_event_id;
use crate::warehouse_metadata::constants::{MAX_HISTORY_RECORDS, STATISTICS_RETENTION_DAYS};
use crate::warehouse_metadata::models::{
    StatisticsObservation, StatisticsPage, StatisticsQuery, WarehouseCatalogRecord,
    WarehouseReconciliationCursor,
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

/// Expiration is bounded global cleanup, never a lifecycle status or warehouse write.
pub(crate) async fn prune_statistics(store: &ControlStore) -> Result<usize, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let cutoff = transaction.now - Duration::days(STATISTICS_RETENTION_DAYS);
    let (transaction, rows) = query(transaction,
        "DELETE FROM sqb_warehouse_statistics WHERE event_id IN (SELECT event_id FROM sqb_warehouse_statistics \
         WHERE observed_at<$1 ORDER BY observed_at,event_id LIMIT $2) RETURNING event_id",
        vec![Parameter::Time(cutoff), Parameter::Integer(MAX_HISTORY_RECORDS as i64)]).await?;
    commit(transaction).await?;
    Ok(rows.len())
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
        let placeholders = (1..=candidates.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(",");
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
    options: &StatisticsQuery,
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
    let cutoff = transaction.now - Duration::days(STATISTICS_RETENTION_DAYS);
    let mut parameters = vec![
        Parameter::Id(location_id),
        Parameter::Text(target.into()),
        Parameter::Text(options.schema.clone()),
        Parameter::Text(options.name.clone()),
        Parameter::Time(cutoff),
    ];
    let database = if let Some(database) = &options.database {
        parameters.push(Parameter::Text(database.clone()));
        "state_database=$6"
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
    let (transaction, rows) = query(transaction,
        &format!("SELECT event_id,record_json AS record FROM sqb_warehouse_statistics \
            WHERE location_id=$1 AND target=$2 AND state_schema=$3 AND relation_name=$4 AND observed_at>=$5 \
            AND {database}{after} ORDER BY observed_at,event_id LIMIT ${}", parameters.len()), parameters).await?;
    let more = rows.len() == limit;
    let records: Vec<StatisticsObservation> = rows
        .into_iter()
        .map(|row| serde_json::from_value(json!(row)))
        .collect::<Result<_, _>>()
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let next_after =
        more.then(|| records.last())
            .flatten()
            .map(|observation| WarehouseReconciliationCursor {
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

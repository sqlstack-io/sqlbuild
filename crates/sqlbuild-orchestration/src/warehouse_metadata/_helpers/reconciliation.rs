//! Immutable scan receipts authorize only bounded, namespace/day-scoped mirror reconciliation.

use chrono::NaiveTime;
use serde_json::{Value, json};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{
    WarehouseReconciliationCursor, WarehouseReconciliationPage, WarehouseSyncResponse,
};

pub(crate) async fn begin_reconciliation(
    mut transaction: ControlTransaction,
    scope: (&str, Uuid, &str),
    page: &WarehouseReconciliationPage,
    mut batch: Value,
) -> Result<ControlTransaction, ControlError> {
    if page.state_schema.trim().is_empty()
        || page.day.is_some_and(|day| day.succ_opt().is_none())
        || (!page.complete && page.after.is_some())
        || page.after.as_ref().is_some_and(|after| {
            page.day
                .is_some_and(|day| after.observed_at.date_naive() != day)
        })
    {
        return Err(ControlError::Invalid(
            "invalid reconciliation namespace, day or continuation".into(),
        ));
    }
    let scope_hash = sha256_hex(
        json!((
            scope,
            &batch["session_id"],
            &batch["generation"],
            &page.state_database,
            &page.state_schema,
            page.day
        ))
        .to_string()
        .as_bytes(),
    );
    batch["reconciliation"]
        .as_object_mut()
        .ok_or_else(|| ControlError::Invalid("missing reconciliation page".into()))?
        .remove("after");
    let request_hash = sha256_hex(batch.to_string().as_bytes());
    let latest;
    (transaction, latest) = query(transaction,
        "SELECT page,scope_hash,complete FROM sqb_warehouse_scan_pages WHERE scan_id=$1 ORDER BY page DESC LIMIT 1",
        vec![Parameter::Id(page.scan_id)]).await?;
    if latest
        .first()
        .is_some_and(|row| row["scope_hash"] != scope_hash)
    {
        return Err(ControlError::Invalid(
            "reconciliation scan belongs to another scope or reader generation".into(),
        ));
    }
    let previous;
    (transaction, previous) = query(
        transaction,
        "SELECT request_hash FROM sqb_warehouse_scan_pages WHERE scan_id=$1 AND page=$2",
        vec![
            Parameter::Id(page.scan_id),
            Parameter::Integer(i64::from(page.page)),
        ],
    )
    .await?;
    if let Some(previous) = previous.first() {
        if previous["request_hash"] != request_hash {
            return Err(ControlError::Invalid(
                "reconciliation page retry changed its content".into(),
            ));
        }
        return Ok(transaction);
    }
    let expected = latest.first().map_or(Some(0), |row| {
        row["page"].as_u64().and_then(|page| page.checked_add(1))
    });
    if expected != Some(u64::from(page.page))
        || page.after.is_some()
        || latest
            .first()
            .is_some_and(|row| row["complete"].as_bool() == Some(true))
    {
        return Err(ControlError::Invalid(
            "reconciliation pages must be contiguous and cannot extend a completed scan".into(),
        ));
    }
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_warehouse_scan_pages (scan_id,page,scope_hash,request_hash,complete,received_at) VALUES ($1,$2,$3,$4,$5,$6)",
        vec![Parameter::Id(page.scan_id), Parameter::Integer(i64::from(page.page)), Parameter::Text(scope_hash),
            Parameter::Text(request_hash), Parameter::Boolean(page.complete), Parameter::Time(now)]).await?;
    Ok(transaction)
}

/// Each request visits at most one mirror page, including retained and concurrently received rows.
pub(crate) async fn finish_reconciliation(
    mut transaction: ControlTransaction,
    (table, location_id, target): (&str, Uuid, &str),
    page: &WarehouseReconciliationPage,
    keys: &[Uuid],
) -> Result<(ControlTransaction, WarehouseSyncResponse), ControlError> {
    for key in keys {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_warehouse_scan_keys (scan_id,event_id) VALUES ($1,$2) ON CONFLICT (scan_id,event_id) DO NOTHING",
            vec![Parameter::Id(page.scan_id), Parameter::Id(*key)]).await?;
    }
    let mut response = WarehouseSyncResponse {
        applied: false,
        next_reconciliation_after: None,
    };
    if !page.complete {
        return Ok((transaction, response));
    }
    let mut parameters = vec![
        Parameter::Id(location_id),
        Parameter::Text(target.into()),
        Parameter::Text(page.state_schema.clone()),
    ];
    let mut bounds = vec![match &page.state_database {
        Some(database) => {
            parameters.push(Parameter::Text(database.clone()));
            format!("state_database=${}", parameters.len())
        }
        None => "state_database IS NULL".into(),
    }];
    if let Some(cursor) = &page.after {
        let offset = parameters.len();
        parameters.extend([
            Parameter::Time(cursor.observed_at),
            Parameter::Id(cursor.event_id),
        ]);
        bounds.push(format!(
            "(observed_at,event_id)>(${},${})",
            offset + 1,
            offset + 2
        ));
    } else if let Some(day) = page.day {
        parameters.push(Parameter::Time(day.and_time(NaiveTime::MIN).and_utc()));
        bounds.push(format!("observed_at>=${}", parameters.len()));
    }
    if let Some(day) = page.day {
        let end = day
            .succ_opt()
            .ok_or_else(|| ControlError::Invalid("invalid reconciliation day".into()))?;
        parameters.push(Parameter::Time(end.and_time(NaiveTime::MIN).and_utc()));
        bounds.push(format!("observed_at<${}", parameters.len()));
    }
    let candidates;
    (transaction, candidates) = query(transaction, &format!(
        "SELECT observed_at,event_id FROM {table} WHERE location_id=$1 AND target=$2 AND state_schema=$3 \
         AND {} ORDER BY observed_at,event_id LIMIT {MAX_HISTORY_RECORDS}", bounds.join(" AND ")), parameters).await?;
    if candidates.len() == MAX_HISTORY_RECORDS {
        response.next_reconciliation_after = Some(
            serde_json::from_value::<WarehouseReconciliationCursor>(json!(candidates.last()))
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        );
    }
    if candidates.is_empty() {
        return Ok((transaction, response));
    }
    let mut parameters = vec![Parameter::Id(page.scan_id)];
    for candidate in &candidates {
        let cursor: WarehouseReconciliationCursor = serde_json::from_value(json!(candidate))
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        parameters.push(Parameter::Id(cursor.event_id));
    }
    let placeholders = (2..=parameters.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(",");
    let removed;
    (transaction, removed) = query(transaction, &format!(
        "DELETE FROM {table} WHERE event_id IN ({placeholders}) \
         AND received_at<(SELECT received_at FROM sqb_warehouse_scan_pages WHERE scan_id=$1 AND page=0) \
         AND NOT EXISTS (SELECT 1 FROM sqb_warehouse_scan_keys k WHERE k.scan_id=$1 AND k.event_id={table}.event_id) RETURNING event_id"), parameters).await?;
    response.applied = !removed.is_empty();
    Ok((transaction, response))
}

//! Ordered, at-least-once delivery; a response is acknowledged only after HTTP success, and a platform event that can't be read is skipped and recorded as a problem.

use uuid::Uuid;

use crate::platform::main::health::record_skipped_event::record_skipped_event;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::webhooks::_helpers::events::read_events;
use crate::webhooks::models::Webhook;

/// A lost acknowledgement may resend the same event_id; receivers deduplicate by that ID.
pub(crate) async fn deliver_webhook(
    store: &ControlStore,
    sink: &Webhook,
) -> Result<usize, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let table = sink.source.deliveries();
    let (transaction, rows) = query(
        transaction,
        &format!("SELECT COALESCE(MAX(storage_id), 0) AS cursor FROM {table} WHERE sink_id=$1"),
        vec![Parameter::Text(sink.sink_id.clone())],
    )
    .await?;
    let cursor = serde_json::from_value(rows[0]["cursor"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, events) = read_events(transaction, sink.source, cursor).await?;
    commit(transaction).await?;
    let count = events.len();
    for event in events {
        let event = match event {
            Ok(event) => event,
            Err(row) => {
                let identity = (row.event.storage_id, row.event.event_id);
                let transaction = begin(store, TransactionMode::Write).await?;
                let transaction = record_skipped_event(transaction, *row).await?;
                commit(acknowledge(transaction, sink, identity).await?).await?;
                continue;
            }
        };
        let (storage_id, event_id) = event.identity();
        let response = sink
            .client
            .post(sink.url.clone())
            .header("Idempotency-Key", event_id.to_string())
            .json(&event)
            .send()
            .await
            .map_err(|error| ControlError::Invalid(transport_failure(&error).into()))?;
        if !response.status().is_success() {
            return Err(ControlError::Invalid(format!(
                "the webhook answered HTTP {}",
                response.status().as_u16()
            )));
        }
        let transaction = begin(store, TransactionMode::Write).await?;
        commit(acknowledge(transaction, sink, (storage_id, event_id)).await?).await?;
    }
    Ok(count)
}

/// Move the sink's cursor past `storage_id`: delivered, or skipped as unreadable and recorded as a problem.
async fn acknowledge(
    transaction: ControlTransaction,
    sink: &Webhook,
    (storage_id, event_id): (i64, Uuid),
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    let table = sink.source.deliveries();
    let (transaction, _) = query(
        transaction,
        &format!(
            "INSERT INTO {table} (sink_id, storage_id, event_id, delivered_at) \
             VALUES ($1,$2,$3,$4) ON CONFLICT (sink_id, storage_id) DO NOTHING"
        ),
        vec![
            Parameter::Text(sink.sink_id.clone()),
            Parameter::Integer(storage_id),
            Parameter::Text(event_id.to_string()),
            Parameter::Time(now),
        ],
    )
    .await?;
    Ok(transaction)
}

/// A failed request in words that never name the endpoint, which may hold credentials.
fn transport_failure(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "the webhook didn't answer in time"
    } else if error.is_connect() {
        "can't connect to the webhook"
    } else {
        "the request to the webhook failed"
    }
}

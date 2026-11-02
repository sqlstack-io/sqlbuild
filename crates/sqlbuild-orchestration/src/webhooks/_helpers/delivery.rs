//! Ordered, at-least-once delivery; a response is acknowledged only after HTTP success.

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
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
        let (storage_id, event_id) = event.identity();
        let response = sink
            .client
            .post(sink.url.clone())
            .header("Idempotency-Key", event_id.to_string())
            .json(&event)
            .send()
            .await
            .map_err(|_| ControlError::Invalid("webhook delivery failed; event retained".into()))?;
        if !response.status().is_success() {
            return Err(ControlError::Invalid(format!(
                "webhook returned HTTP {}; event retained",
                response.status().as_u16()
            )));
        }
        let transaction = begin(store, TransactionMode::Write).await?;
        let now = transaction.now;
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
        commit(transaction).await?;
    }
    Ok(count)
}

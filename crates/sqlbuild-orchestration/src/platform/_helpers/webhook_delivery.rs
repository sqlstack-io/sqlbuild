//! Ordered, at-least-once delivery; a response is acknowledged only after HTTP success.

use crate::platform::constants::SINK_PAGE_SIZE;
use crate::platform::models::{PlatformEventEnvelope, PlatformWebhook};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// A lost acknowledgement may resend the same event_id; receivers must deduplicate by that ID.
pub(crate) async fn deliver_platform_events(
    store: &ControlStore,
    sink: &PlatformWebhook,
) -> Result<usize, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT storage_id, event_id, occurred_at, payload FROM recall_events \
         WHERE producer='platform' AND invocation_id LIKE 'platform/%' AND storage_id > \
         (SELECT COALESCE(MAX(storage_id), 0) FROM sqb_platform_sink_deliveries WHERE sink_id=$1) \
         ORDER BY storage_id LIMIT $2",
        vec![
            Parameter::Text(sink.sink_id.clone()),
            Parameter::Integer(SINK_PAGE_SIZE),
        ],
    )
    .await?;
    let events: Vec<PlatformEventEnvelope> =
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    commit(transaction).await?;
    let count = events.len();
    for event in events {
        let response = sink
            .client
            .post(sink.url.clone())
            .header("Idempotency-Key", event.event_id.to_string())
            .json(&event)
            .send()
            .await
            .map_err(|_| {
                ControlError::Invalid("platform webhook delivery failed; event retained".into())
            })?;
        if !response.status().is_success() {
            return Err(ControlError::Invalid(format!(
                "platform webhook returned HTTP {}; event retained",
                response.status().as_u16()
            )));
        }
        let transaction = begin(store, TransactionMode::Write).await?;
        let now = transaction.now;
        let (transaction, _) = query(transaction,
            "INSERT INTO sqb_platform_sink_deliveries (sink_id, storage_id, event_id, delivered_at) \
             VALUES ($1,$2,$3,$4) ON CONFLICT (sink_id, storage_id) DO NOTHING",
            vec![Parameter::Text(sink.sink_id.clone()), Parameter::Integer(event.storage_id),
                Parameter::Text(event.event_id.to_string()), Parameter::Time(now)]).await?;
        commit(transaction).await?;
    }
    Ok(count)
}

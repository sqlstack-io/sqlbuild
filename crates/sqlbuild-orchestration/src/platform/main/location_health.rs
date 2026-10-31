//! Read bounded recent observations and instance liveness; consumers derive freshness.

use uuid::Uuid;

use crate::platform::models::LocationHealthObservation;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn location_health(
    store: &ControlStore,
    location_id: Uuid,
) -> Result<Vec<LocationHealthObservation>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let today = transaction.now.date_naive();
    let yesterday = today - chrono::TimeDelta::days(1);
    let (transaction, rows) = query(
        transaction,
        "SELECT received_at, payload FROM (\
            SELECT * FROM (SELECT occurred_at AS received_at, invocation_sequence, payload FROM recall_events \
                WHERE invocation_id=$1 AND event_type='location_health' ORDER BY invocation_sequence DESC LIMIT 100) current_day \
            UNION ALL \
            SELECT * FROM (SELECT occurred_at AS received_at, invocation_sequence, payload FROM recall_events \
                WHERE invocation_id=$2 AND event_type='location_health' ORDER BY invocation_sequence DESC LIMIT 100) previous_day\
         ) observations ORDER BY received_at DESC, invocation_sequence DESC LIMIT 100",
        vec![
            Parameter::Text(format!("location/{location_id}/{today}")),
            Parameter::Text(format!("location/{location_id}/{yesterday}")),
        ],
    )
    .await?;
    let mut observations: Vec<LocationHealthObservation> = rows
        .into_iter()
        .map(|mut row| {
            let report = row
                .remove("payload")
                .and_then(|mut payload| payload.get_mut("report").map(serde_json::Value::take))
                .ok_or_else(|| ControlError::Invalid("location health report is missing".into()))?;
            row.insert("report".into(), report);
            decode_row(row)
        })
        .collect::<Result<_, _>>()?;
    let (transaction, instances) = query(
        transaction,
        "SELECT last_seen_at AS received_at, health_json AS report FROM sqb_location_instances \
         WHERE location_id=$1 ORDER BY last_seen_at DESC LIMIT 100",
        vec![Parameter::Id(location_id)],
    )
    .await?;
    observations.extend(
        instances
            .into_iter()
            .map(decode_row)
            .collect::<Result<Vec<LocationHealthObservation>, _>>()?,
    );
    observations.sort_by_key(|observation| std::cmp::Reverse(observation.received_at));
    observations.truncate(100);
    commit(transaction).await?;
    Ok(observations)
}

//! The newest health observations of a location: reported events and instance liveness rows.

use uuid::Uuid;

use crate::platform::models::LocationHealthObservation;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// At most 100, newest first, from today's and yesterday's reports plus each instance's last check-in, of one target when given.
pub(crate) async fn recent_health(
    transaction: ControlTransaction,
    location_id: Uuid,
    target: Option<&str>,
) -> Result<(ControlTransaction, Vec<LocationHealthObservation>), ControlError> {
    let today = transaction.now.date_naive();
    let yesterday = today - chrono::TimeDelta::days(1);
    let (event_target, instance_target) = match target {
        Some(_) => (" AND payload->'report'->>'target'=$3", " AND target=$2"),
        None => ("", ""),
    };
    let target_parameter: Vec<Parameter> = target
        .map(|target| Parameter::Text(target.to_owned()))
        .into_iter()
        .collect();
    let (transaction, rows) = query(
        transaction,
        &format!("SELECT received_at, payload FROM (\
            SELECT * FROM (SELECT occurred_at AS received_at, invocation_sequence, payload FROM recall_events \
                WHERE invocation_id=$1 AND event_type='location_health'{event_target} ORDER BY invocation_sequence DESC LIMIT 100) current_day \
            UNION ALL \
            SELECT * FROM (SELECT occurred_at AS received_at, invocation_sequence, payload FROM recall_events \
                WHERE invocation_id=$2 AND event_type='location_health'{event_target} ORDER BY invocation_sequence DESC LIMIT 100) previous_day\
         ) observations ORDER BY received_at DESC, invocation_sequence DESC LIMIT 100"),
        [
            Parameter::Text(format!("location/{location_id}/{today}")),
            Parameter::Text(format!("location/{location_id}/{yesterday}")),
        ]
        .into_iter()
        .chain(target_parameter.clone())
        .collect(),
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
        &format!(
            "SELECT last_seen_at AS received_at, health_json AS report FROM sqb_location_instances \
         WHERE location_id=$1{instance_target} ORDER BY last_seen_at DESC LIMIT 100"
        ),
        std::iter::once(Parameter::Id(location_id))
            .chain(target_parameter)
            .collect(),
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
    Ok((transaction, observations))
}

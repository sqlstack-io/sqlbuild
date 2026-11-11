//! The newest health observations of a location: reported events and instance liveness rows.

use uuid::Uuid;

use crate::platform::constants::LOCATION_RETENTION_DAYS;
use crate::platform::models::LocationHealthObservation;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// At most 100, newest first, from every retained day (a sync follows only runs) plus each instance's last check-in.
pub(crate) async fn recent_health(
    transaction: ControlTransaction,
    location_id: Uuid,
    target: Option<&str>,
) -> Result<(ControlTransaction, Vec<LocationHealthObservation>), ControlError> {
    let today = transaction.now.date_naive();
    let days = 0..=LOCATION_RETENTION_DAYS;
    let target_placeholder = days.clone().count() + 1;
    let (event_target, instance_target) = match target {
        Some(_) => (
            format!(" AND payload->'report'->>'target'=${target_placeholder}"),
            " AND target=$2",
        ),
        None => (String::new(), ""),
    };
    let target_parameter: Vec<Parameter> = target
        .map(|target| Parameter::Text(target.to_owned()))
        .into_iter()
        .collect();
    let daily = days
        .clone()
        .map(|day| {
            format!("SELECT * FROM (SELECT occurred_at AS received_at, invocation_sequence, payload FROM recall_events \
                WHERE invocation_id=${} AND event_type='location_health'{event_target} ORDER BY invocation_sequence DESC LIMIT 100) day_{day}", day + 1)
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT received_at, payload FROM ({daily}) observations \
            ORDER BY received_at DESC, invocation_sequence DESC LIMIT 100"
        ),
        days.map(|day| {
            Parameter::Text(format!(
                "location/{location_id}/{}",
                today - chrono::TimeDelta::days(day)
            ))
        })
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

//! Checking-in locations whose latest load, plan of what changed, discovery or connection check failed.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::discovered_sets::models::DiscoveredSetStatus;
use crate::platform::_helpers::health::recent_health::recent_health;
use crate::platform::constants::LOCATION_SILENCE_SECONDS;
use crate::platform::models::{
    DiscoveryHealth, FailingLocation, LocationHealth, LocationHealthObservation, LocationState,
};
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The fact a degraded location reports, which carries no text of its own.
const CONNECTION_FAILED: &str = "the warehouse connection check after its last load failed";

/// Every live location target whose state is a failure, with the report that decided it and the snapshot it serves.
pub(crate) async fn failing_locations(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, Vec<FailingLocation>), ControlError> {
    let now = transaction.now;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT DISTINCT i.location_id, l.name AS location_name, i.target \
         FROM sqb_location_instances i JOIN sqb_locations l ON l.location_id=i.location_id \
         WHERE l.archived_at IS NULL AND i.last_seen_at>=$1 ORDER BY l.name, i.target",
        vec![Parameter::Time(
            now - TimeDelta::seconds(LOCATION_SILENCE_SECONDS),
        )],
    )
    .await?;
    let mut failing: Vec<FailingLocation> = Vec::new();
    for row in rows {
        let LiveTarget {
            location_id,
            location_name,
            target,
        } = decode_row(row)?;
        let target: &str = &target;
        let observations;
        (transaction, observations) = recent_health(transaction, location_id, Some(target)).await?;
        let state = LocationState::of(&observations, (target, now));
        let Some((failed_at, error)) = deciding_report(&observations, state) else {
            continue;
        };
        let serving;
        (transaction, serving) = read_current_record(transaction, (location_id, target)).await?;
        failing.push(FailingLocation {
            location_id,
            location_name,
            target: target.to_owned(),
            state,
            failed_at,
            error,
            serving,
        });
    }
    Ok((transaction, failing))
}

/// When the failure that decided `state` was reported and its text; `None` for a state that isn't a failure here.
fn deciding_report(
    observations: &[LocationHealthObservation],
    state: LocationState,
) -> Option<(chrono::DateTime<chrono::Utc>, Option<String>)> {
    observations.iter().find_map(|observation| {
        let at = observation.received_at;
        match (&observation.report.health, state) {
            (LocationHealth::CompileFailed { error, .. }, LocationState::FallingBack) => {
                Some((at, error.clone()))
            }
            (LocationHealth::ChangePlanFailed { reason, .. }, LocationState::ChangePlanFailed) => {
                Some((at, Some(reason.clone())))
            }
            (LocationHealth::Discovery { sets, .. }, LocationState::DiscoveryFailed) => {
                Some((at, Some(failed_sets(sets)?)))
            }
            (LocationHealth::Connection { healthy: false, .. }, LocationState::Degraded) => {
                Some((at, Some(CONNECTION_FAILED.to_owned())))
            }
            _ => None,
        }
    })
}

/// `name: error` of each failed set, joined; `None` when none failed.
fn failed_sets(sets: &[DiscoveryHealth]) -> Option<String> {
    let mut failed: Vec<String> = Vec::new();
    for set in sets {
        if set.status != DiscoveredSetStatus::Failed {
            continue;
        }
        failed.push(match &set.error {
            Some(error) => format!("{}: {error}", set.name),
            None => set.name.clone(),
        });
    }
    (!failed.is_empty()).then(|| failed.join("; "))
}

/// A live location target, as the query reads it.
#[derive(serde::Deserialize)]
struct LiveTarget {
    location_id: Uuid,
    location_name: String,
    target: String,
}

//! A location's run-history sync as its newest reports describe it: running since when, stalled, or failing since when, why, and when it tries again.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::platform::constants::SYNC_STALL_SECONDS;
use crate::platform::models::{
    FailingSync, LocationFact, LocationHealth, LocationHealthObservation, RunningSync, StalledSync,
    WarehouseReadFailure, WarehouseSyncFailure, WarehouseSyncProgress, WarehouseSyncState,
};

/// The sync of `target` at `now` from its newest report: running since when and stalled, or failing; not the request.
pub(crate) fn sync_state(
    observations: &[LocationHealthObservation],
    (target, now): (&str, DateTime<Utc>),
) -> WarehouseSyncState {
    let newest = observations
        .iter()
        .filter(|observation| observation.report.target == target)
        .find_map(|observation| match &observation.report.health {
            LocationHealth::WarehouseSync {
                failure,
                progress,
                table,
                ..
            } => Some((observation.received_at, *failure, *progress, *table)),
            _ => None,
        });
    let Some((at, failure, progress, table)) = newest else {
        return WarehouseSyncState::default();
    };
    let halted = progress.is_some_and(|progress| progress.halted);
    let failing = halted || failure.is_some_and(|failure| failure != WarehouseSyncFailure::Busy);
    let running = progress
        .filter(|progress| failure.is_none() && !progress.completed && !progress.halted)
        .map(|progress| RunningSync {
            started_at: at
                - TimeDelta::milliseconds(i64::try_from(progress.elapsed_millis).unwrap_or(0)),
            progressed_at: at,
            position: progress.position,
            kinds: progress.kinds,
            rows: progress.rows,
            table,
            stalled: now - at >= TimeDelta::seconds(SYNC_STALL_SECONDS),
        });
    WarehouseSyncState {
        requested: None,
        running,
        failing,
    }
}

/// The sync of `place` while it is running and stalled.
pub(crate) fn stalled_sync(
    observations: &[LocationHealthObservation],
    (location_id, location_name, target): (Uuid, &str, &str),
    now: DateTime<Utc>,
) -> Option<StalledSync> {
    let running = sync_state(observations, (target, now))
        .running
        .filter(|running| running.stalled)?;
    Some(StalledSync {
        location_id,
        location_name: location_name.to_owned(),
        target: target.to_owned(),
        progressed_at: running.progressed_at,
        position: running.position,
        kinds: running.kinds,
        table: running.table,
        tier: LocationFact::SyncStalled.problem_tier(),
    })
}

/// The sync of `place`, newest report first, while its newest report is a failure; a waiting sync isn't one.
pub(crate) fn failing_sync(
    observations: &[LocationHealthObservation],
    (location_id, location_name, target): (Uuid, &str, &str),
) -> Option<FailingSync> {
    let syncs: Vec<(&LocationHealthObservation, Option<WarehouseSyncFailure>)> = observations
        .iter()
        .filter(|observation| observation.report.target == target)
        .filter_map(|observation| match &observation.report.health {
            LocationHealth::WarehouseSync { failure, .. } => Some((observation, *failure)),
            _ => None,
        })
        .collect();
    let (newest, failure) = *syncs.first()?;
    let failure = failure.filter(|failure| *failure != WarehouseSyncFailure::Busy)?;
    let LocationHealth::WarehouseSync {
        error,
        read_failure,
        progress,
        failing_since,
        retry_at,
        retry_seconds,
        ..
    } = &newest.report.health
    else {
        return None;
    };
    let failing: Vec<&LocationHealthObservation> = syncs
        .iter()
        .take_while(|(_, failure)| {
            failure.is_some_and(|failure| failure != WarehouseSyncFailure::Busy)
        })
        .map(|(observation, _)| *observation)
        .collect();
    let first_reported = failing
        .last()
        .map_or(newest.received_at, |observation| observation.received_at);
    let halted: bool = progress
        .as_ref()
        .is_some_and(|progress: &WarehouseSyncProgress| progress.halted);
    Some(FailingSync {
        location_id,
        location_name: location_name.to_owned(),
        target: target.to_owned(),
        since: failing_since.map_or(first_reported, |since| since.min(first_reported)),
        reason: error
            .clone()
            .unwrap_or_else(|| stage_words(failure).to_owned()),
        unreachable: matches!(read_failure, Some(WarehouseReadFailure::Unreachable {})),
        halted,
        retry_at: retry_at.filter(|_| !halted),
        retry_seconds: retry_seconds.filter(|_| !halted),
        last_success_at: syncs
            .iter()
            .find(|(_, failure)| failure.is_none())
            .map(|(observation, _)| observation.received_at),
        tier: LocationFact::SyncFailing.problem_tier(),
    })
}

/// What failed when the report has no error text of its own.
fn stage_words(failure: WarehouseSyncFailure) -> &'static str {
    match failure {
        WarehouseSyncFailure::Watermark => {
            "the location couldn't read where the sync had got to from the server"
        }
        WarehouseSyncFailure::Read => "the warehouse read failed",
        WarehouseSyncFailure::Delivery => "the location couldn't deliver history to the server",
        WarehouseSyncFailure::Busy => "the warehouse file is in use",
    }
}

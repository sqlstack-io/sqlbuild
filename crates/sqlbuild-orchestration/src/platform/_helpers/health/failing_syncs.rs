//! A location's run-history sync as its newest reports describe it: failing since when, why, and when it tries again.

use uuid::Uuid;

use crate::platform::models::{
    FailingSync, LocationHealth, LocationHealthObservation, WarehouseReadFailure,
    WarehouseSyncFailure, WarehouseSyncProgress,
};

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

//! Liveness is derived across all sessions; an old instance cannot mask a healthy successor, and work for a silent target is refused.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::platform::_helpers::health::failing_locations::failing_locations;
use crate::platform::_helpers::health::observations::observe;
use crate::platform::constants::LOCATION_SILENCE_SECONDS;
use crate::platform::models::{LastCheckIn, PlatformEvent, WatchdogLocation};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

/// Locations report every 30 seconds; tolerate two missed reports.
pub(crate) async fn scan_location_page(
    store: &ControlStore,
    installation_id: Uuid,
    after: Option<(Uuid, String)>,
) -> Result<Option<(Uuid, String)>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let mut parameters = vec![Parameter::Time(
        transaction.now - TimeDelta::seconds(LOCATION_SILENCE_SECONDS),
    )];
    let cursor = if let Some((location_id, target)) = after {
        parameters.extend([Parameter::Id(location_id), Parameter::Text(target)]);
        "AND (i.location_id>$2 OR (i.location_id=$2 AND i.target>$3))"
    } else {
        ""
    };
    let (mut transaction, rows) = query(
        transaction,
        &format!(
            "SELECT i.location_id, i.target, MAX(i.last_seen_at) AS last_seen_at \
         FROM sqb_location_instances i JOIN sqb_locations l ON l.location_id=i.location_id \
         WHERE l.archived_at IS NULL {cursor} GROUP BY i.location_id,i.target \
         HAVING MAX(i.last_seen_at)<$1 ORDER BY i.location_id,i.target LIMIT 100"
        ),
        parameters,
    )
    .await?;
    let locations: Vec<WatchdogLocation> =
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    let next = locations
        .last()
        .map(|row| (row.location_id, row.target.clone()));
    for location in locations {
        transaction = observe(
            transaction,
            installation_id,
            (
                PlatformEvent::LocationBroken {
                    location_id: location.location_id,
                    target: location.target,
                    last_seen_at: location.last_seen_at,
                },
                None,
            ),
        )
        .await?;
    }
    commit(transaction).await?;
    Ok(next)
}

/// Every failing location once per failure: the time it was reported tells one failure from the next.
pub(crate) async fn observe_failing_locations(
    store: &ControlStore,
    installation_id: Uuid,
) -> Result<(), ControlError> {
    let (mut transaction, failing) =
        failing_locations(begin(store, TransactionMode::Write).await?).await?;
    for location in failing.locations {
        let incident = Some(location.failed_at);
        transaction = observe(
            transaction,
            installation_id,
            (
                PlatformEvent::LocationFailing {
                    location_id: location.location_id,
                    target: location.target,
                    state: location.state,
                    failed_at: location.failed_at,
                    error: location.error,
                    serving_generation: location.serving.map(|snapshot| snapshot.generation),
                },
                incident,
            ),
        )
        .await?;
    }
    commit(transaction).await?;
    Ok(())
}

/// Refuse work for a target that isn't checking in, at once and saying since when; `what` is what it can't do.
pub(crate) async fn require_checking_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    what: &str,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "SELECT MAX(last_seen_at) AS last_seen_at FROM sqb_location_instances WHERE location_id=$1 AND target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.to_owned())],
    )
    .await?;
    let seen: Option<LastCheckIn> = rows.into_iter().next().map(decode_row).transpose()?;
    match seen.and_then(|seen| seen.last_seen_at) {
        Some(at) if now - at <= TimeDelta::seconds(LOCATION_SILENCE_SECONDS) => Ok(transaction),
        Some(at) => Err(ControlError::NotCheckingIn(format!(
            "The location isn't checking in (last check-in {} ago), so it can't {what}. Start it, then try again.",
            silence_words(now - at)
        ))),
        None => Err(ControlError::NotCheckingIn(format!(
            "The location hasn't checked in yet, so it can't {what}. Start it, then try again."
        ))),
    }
}

/// How long a location has been silent, short: `6m`, `3h`, `2d`.
fn silence_words(silence: TimeDelta) -> String {
    if silence < TimeDelta::hours(1) {
        format!("{}m", silence.num_minutes().max(1))
    } else if silence < TimeDelta::days(1) {
        format!("{}h", silence.num_hours())
    } else {
        format!("{}d", silence.num_days())
    }
}

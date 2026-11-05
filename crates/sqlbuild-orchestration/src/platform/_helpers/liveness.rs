//! Liveness is derived across all sessions; an old instance cannot mask a healthy successor.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::platform::_helpers::observations::observe;
use crate::platform::models::{PlatformEvent, WatchdogLocation};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Locations report every 30 seconds; tolerate two missed reports.
pub(crate) async fn scan_location_page(
    store: &ControlStore,
    installation_id: Uuid,
    after: Option<(Uuid, String)>,
) -> Result<Option<(Uuid, String)>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let mut parameters = vec![Parameter::Time(transaction.now - TimeDelta::seconds(90))];
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

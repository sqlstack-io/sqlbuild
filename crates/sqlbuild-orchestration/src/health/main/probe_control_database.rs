//! Check the control database every few seconds, and read which locations there are, so health is current without a request.

use crate::platform::main::health::report_session_cut_offs::report_session_cut_offs;
use chrono::Utc;
use tokio::sync::watch;

use crate::health::constants::HEALTH_PROBE_INTERVAL;
use crate::health::main::control_cause::control_cause;
use crate::health::main::log_cause::log_cause;
use crate::health::models::{ServerHealth, StoredLocationReport};
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Probe `store` until `stopped` turns true, recording each result in `health`.
pub async fn probe_control_database(
    store: ControlStore,
    health: ServerHealth,
    mut stopped: watch::Receiver<bool>,
) {
    health.attach_store(&store);
    let mut failing = false;
    while !*stopped.borrow() {
        let cut_off = match report_session_cut_offs(&store, &health).await {
            Ok(cut_off) => cut_off,
            Err(error) => {
                if let Some(cause) = control_cause(&error, (&Default::default(), false)) {
                    log_cause(None, &health.locate(cause));
                }
                true
            }
        };
        match active_locations(&store).await {
            Ok(locations) => {
                if !cut_off {
                    health.control_succeeded(Utc::now());
                }
                health.locations_seen(locations);
                failing = false;
            }
            Err(error) => {
                if let Some(cause) = control_cause(&error, (&Default::default(), false)) {
                    let cause = health.locate(cause);
                    if cause.unavailable() {
                        health.control_failed(&cause, (Utc::now(), None));
                    }
                    if !failing {
                        log_cause(None, &cause);
                    }
                    failing = true;
                }
            }
        }
        tokio::select! {
            () = tokio::time::sleep(HEALTH_PROBE_INTERVAL) => {},
            changed = stopped.changed() => if changed.is_err() { break },
        }
    }
}

/// Every active location with its instances' newest report.
async fn active_locations(store: &ControlStore) -> Result<Vec<StoredLocationReport>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, locations) = query_records(
        transaction,
        "SELECT l.location_id, l.name, \
         (SELECT MAX(i.last_seen_at) FROM sqb_location_instances i WHERE i.location_id=l.location_id) AS last_seen_at \
         FROM sqb_locations l WHERE l.archived_at IS NULL ORDER BY l.name",
        Vec::new(),
    )
    .await?;
    commit(transaction).await?;
    Ok(locations)
}

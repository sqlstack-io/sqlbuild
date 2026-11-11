//! One maintenance role per server: health alerts every cycle, bounded retention every minute.

use std::time::{Duration, Instant};

use tokio::sync::watch;
use uuid::Uuid;

use crate::credentials::models::AuditRetention;
use crate::platform::_helpers::retention::retain;
use crate::platform::_helpers::watchdog::scan_health;
use crate::snapshots::models::SnapshotRetention;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::models::StatisticsRetention;
use crate::webhooks::models::{Webhook, WebhookSource};

/// Only this server's security-audit sinks acknowledge; `report` hears each new retained count.
pub async fn run_maintenance(
    (store, replica_id): (&ControlStore, Uuid),
    (retention, sinks): (
        (AuditRetention, StatisticsRetention, SnapshotRetention),
        &[Webhook],
    ),
    (report, mut shutdown): (impl Fn(i64), watch::Receiver<bool>),
) -> Result<(), ControlError> {
    let sinks: Vec<String> = sinks
        .iter()
        .filter(|sink| matches!(sink.source, WebhookSource::SecurityAudit))
        .map(|sink| sink.sink_id.clone())
        .collect();
    let mut reported = 0;
    let mut retained_at: Option<Instant> = None;
    while !*shutdown.borrow() {
        match scan_health(store, replica_id, &shutdown).await {
            Ok(()) => {}
            Err(error) if error.transient() => {}
            Err(error) => return Err(error),
        }
        if retained_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(60)) {
            match retain(store, retention, &sinks).await {
                Ok(retained) => {
                    if retained != reported {
                        report(retained);
                    }
                    reported = retained;
                }
                Err(error) if error.transient() => {}
                Err(error) => return Err(error),
            }
            retained_at = Some(Instant::now());
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(30)) => {},
            result = shutdown.changed() => { if result.is_err() { break; } },
        }
    }
    Ok(())
}

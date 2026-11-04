//! Expire audit rows periodically and say how many outlive the horizon, and why that may grow.

use std::time::Duration;

use tokio::sync::watch;

use crate::credentials::_helpers::audit_retention::prune_audit;
use crate::credentials::models::AuditRetention;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::webhooks::models::{Webhook, WebhookSource};

/// Only this server's security-audit sinks acknowledge; `report` hears each new retained count.
pub async fn run_audit_retention(
    store: &ControlStore,
    (retention, sinks): (AuditRetention, &[Webhook]),
    (report, mut shutdown): (impl Fn(i64), watch::Receiver<bool>),
) -> Result<(), ControlError> {
    let sinks: Vec<String> = sinks
        .iter()
        .filter(|sink| matches!(sink.source, WebhookSource::SecurityAudit))
        .map(|sink| sink.sink_id.clone())
        .collect();
    let mut reported = 0;
    while !*shutdown.borrow() {
        match prune_audit(store, retention, &sinks).await {
            Ok((_, retained)) => {
                if retained != reported {
                    report(retained);
                }
                reported = retained;
            }
            Err(ControlError::Database(error)) if ControlError::transient_database(&error) => {}
            Err(error) => return Err(error),
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(60)) => {},
            result = shutdown.changed() => { if result.is_err() { break; } },
        }
    }
    Ok(())
}

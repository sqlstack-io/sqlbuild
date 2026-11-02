//! Deliver durable events independently of user interpreters and scheduler leadership.

use std::time::Duration;

use tokio::sync::watch;

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::webhooks::_helpers::delivery::deliver_webhook;
use crate::webhooks::constants::POLL_SECONDS;
use crate::webhooks::models::Webhook;

pub async fn run_webhook(
    store: &ControlStore,
    sink: Webhook,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ControlError> {
    let mut failed = false;
    while !*shutdown.borrow() {
        tokio::select! {
            result = deliver_webhook(store, &sink) => {
                match result {
                    Ok(count) => {
                        if failed {
                            eprintln!("{} webhook delivery recovered", sink.source.label());
                        }
                        failed = false;
                        if count > 0 { continue; }
                    },
                    Err(_) => {
                        if !failed {
                            eprintln!("{} webhook delivery failed; retaining events and retrying", sink.source.label());
                        }
                        failed = true;
                    },
                }
            },
            result = shutdown.changed() => {
                if result.is_err() || *shutdown.borrow() { break; }
            },
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(POLL_SECONDS)) => {},
            result = shutdown.changed() => { if result.is_err() { break; } },
        }
    }
    Ok(())
}

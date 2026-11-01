//! Deliver platform health independently of user interpreters and scheduler leadership.

use std::time::Duration;

use tokio::sync::watch;

use crate::platform::_helpers::webhook_delivery::deliver_platform_events;
use crate::platform::constants::SINK_POLL_SECONDS;
use crate::platform::models::PlatformWebhook;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub async fn run_platform_sink(
    store: &ControlStore,
    sink: PlatformWebhook,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ControlError> {
    let mut failed = false;
    while !*shutdown.borrow() {
        tokio::select! {
            result = deliver_platform_events(store, &sink) => {
                match result {
                    Ok(count) => {
                        if failed {
                            eprintln!("Platform webhook delivery recovered");
                        }
                        failed = false;
                        if count > 0 { continue; }
                    },
                    Err(_) => {
                        if !failed {
                            eprintln!("Platform webhook delivery failed; retaining events and retrying");
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
            _ = tokio::time::sleep(Duration::from_secs(SINK_POLL_SECONDS)) => {},
            result = shutdown.changed() => { if result.is_err() { break; } },
        }
    }
    Ok(())
}

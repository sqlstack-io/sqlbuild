//! Deliver durable events independently of user interpreters and scheduler leadership.

use std::time::Duration;

use tokio::sync::watch;

use crate::health::models::ServerHealth;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::webhooks::_helpers::delivery::deliver_webhook;
use crate::webhooks::constants::POLL_SECONDS;
use crate::webhooks::models::Webhook;

/// Deliver `sink`'s events until `shutdown`, recording each outcome in `health` so a failing webhook is seen.
pub async fn run_webhook(
    (store, health): (ControlStore, Option<ServerHealth>),
    sink: Webhook,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ControlError> {
    let mut failed = false;
    let name: &str = sink.source.forwarding();
    while !*shutdown.borrow() {
        tokio::select! {
            result = deliver_webhook(&store, &sink) => {
                let now = chrono::Utc::now();
                match result {
                    Ok(count) => {
                        if failed {
                            eprintln!("{} webhook delivery recovered", sink.source.label());
                        }
                        failed = false;
                        health.iter().for_each(|health| health.forwarding_succeeded(name, now));
                        if count > 0 { continue; }
                    },
                    Err(error) => {
                        if !failed {
                            eprintln!("{} webhook delivery failed; retaining events and retrying", sink.source.label());
                        }
                        failed = true;
                        let reason = format!(
                            "Can't deliver {} events: {}; they are kept and retried every {POLL_SECONDS} s",
                            sink.source.label().to_lowercase(),
                            failure_text(&error)
                        );
                        health.iter().for_each(|health| health.forwarding_failed(name, (&reason, now)));
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

/// Why delivery failed: the webhook's own failure, or the control database it reads the events from.
fn failure_text(error: &ControlError) -> &str {
    match error {
        ControlError::Invalid(message) => message,
        _ => "the events to forward can't be read from the control database",
    }
}

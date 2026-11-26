//! This server's event webhooks that can't deliver, as live problems.

use crate::health::models::ServerHealth;
use crate::platform::models::FailingForwarding;

/// Each webhook whose newest attempt failed, since its first failure in a row.
#[must_use]
pub fn failing_forwarding(health: &ServerHealth) -> Vec<FailingForwarding> {
    let state = health.state();
    let mut failing: Vec<FailingForwarding> = Vec::new();
    for (name, forwarding) in &state.forwarding {
        let Some(failure) = forwarding
            .last_failure
            .as_ref()
            .filter(|_| forwarding.failing())
        else {
            continue;
        };
        failing.push(FailingForwarding {
            name: name.clone(),
            since: failure.since,
            reason: failure.reason.clone(),
            last_success_at: forwarding.last_success,
        });
    }
    failing
}

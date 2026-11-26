//! A webhook that can't deliver degrades the server from its first failure, with why; a delivery clears it.

use chrono::{DateTime, TimeDelta, Utc};

use crate::health::main::health_report::health_report;
use crate::health::models::{HealthComponent, HealthStatus, ServerHealth};
use crate::health::tests::test_types::ForwardingTestCase;

#[test]
fn given_webhook_outcomes_when_reporting_health_then_failing_forwarding_degrades_the_server() {
    const NAME: &str = "Platform event forwarding";
    const REASON: &str = "Can't deliver platform events: the webhook answered HTTP 503";
    let test_cases = [
        ForwardingTestCase {
            description: "two failures after a delivery",
            outcomes: vec![(0, true), (5, false), (10, false)],
            expected_state: (HealthStatus::Degraded, Some(5), Some(REASON)),
        },
        ForwardingTestCase {
            description: "a delivery after the failures",
            outcomes: vec![(5, false), (10, true)],
            expected_state: (HealthStatus::Healthy, Some(10), None),
        },
    ];
    let start: DateTime<Utc> = "2026-11-25T17:42:00Z".parse().expect("time");
    for test_case in test_cases {
        let health = ServerHealth::new("1.0.0", false);
        health.control_succeeded(start);
        for (seconds, delivered) in &test_case.outcomes {
            let at = start + TimeDelta::seconds(*seconds);
            let outcomes: [fn(&ServerHealth, DateTime<Utc>); 2] = [
                |health, at| health.forwarding_succeeded(NAME, at),
                |health, at| health.forwarding_failed(NAME, (REASON, at)),
            ];
            outcomes[usize::from(!*delivered)](&health, at);
        }
        let report = health_report(&health, start + TimeDelta::seconds(20));
        let forwarding = report
            .components
            .iter()
            .find(|component| component.component == HealthComponent::EventForwarding)
            .expect("a forwarding row");
        assert_eq!(
            (
                report.status,
                forwarding.since.map(|since| (since - start).num_seconds()),
                forwarding.reason.as_deref(),
            ),
            test_case.expected_state,
            "{}",
            test_case.description
        );
    }
}

//! The health report answers from memory: the control database decides unavailable, any other part degraded, and a
//! success after a failure clears it.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::health::main::control_cause::control_cause;
use crate::health::main::health_report::health_report;
use crate::health::models::{HealthStatus, ServerHealth, StoredLocationReport};
use crate::health::tests::helpers::{io, target};
use crate::health::tests::test_types::ReportTestCase;

const DNS: &str = "Can't reach the control database: db.internal doesn't resolve (DNS)";

fn unresolved(health: &ServerHealth, at: DateTime<Utc>) {
    let error = io(
        std::io::ErrorKind::Other,
        "failed to lookup address information: Name or service not known",
    );
    let cause = control_cause(&error, (&target(), false)).expect("a database cause");
    health.control_failed(&cause, (at, None));
}

fn locations(health: &ServerHealth, at: DateTime<Utc>) {
    health.locations_seen(vec![
        StoredLocationReport {
            location_id: Uuid::from_u128(1),
            name: "orders".into(),
            last_seen_at: Some(at - TimeDelta::seconds(20)),
        },
        StoredLocationReport {
            location_id: Uuid::from_u128(2),
            name: "finance".into(),
            last_seen_at: Some(at - TimeDelta::minutes(20)),
        },
    ]);
}

#[test]
fn given_component_events_when_reporting_health_then_the_worst_component_decides() {
    let test_cases = [
        ReportTestCase {
            description: "before the control database is reached",
            events: |_, _| {},
            expected_status: HealthStatus::Unavailable,
            expected_components: vec![
                ("Server", HealthStatus::Healthy, None),
                (
                    "Control database",
                    HealthStatus::Unavailable,
                    Some("Connecting to the control database"),
                ),
                ("Scheduler", HealthStatus::Degraded, Some("Starting")),
            ],
        },
        ReportTestCase {
            description: "everything working",
            events: |health, at| {
                health.control_succeeded(at);
                health.scheduler_succeeded(at);
            },
            expected_status: HealthStatus::Healthy,
            expected_components: vec![
                ("Server", HealthStatus::Healthy, None),
                ("Control database", HealthStatus::Healthy, None),
                ("Scheduler", HealthStatus::Healthy, None),
            ],
        },
        ReportTestCase {
            description: "the control database host stops resolving",
            events: |health, at| {
                health.control_succeeded(at - TimeDelta::minutes(5));
                health.scheduler_succeeded(at - TimeDelta::minutes(5));
                unresolved(health, at);
                health.scheduler_failed("Waiting for the control database", (at, None));
            },
            expected_status: HealthStatus::Unavailable,
            expected_components: vec![
                ("Server", HealthStatus::Healthy, None),
                ("Control database", HealthStatus::Unavailable, Some(DNS)),
                (
                    "Scheduler",
                    HealthStatus::Degraded,
                    Some("Waiting for the control database"),
                ),
            ],
        },
        ReportTestCase {
            description: "the control database recovers",
            events: |health, at| {
                unresolved(health, at - TimeDelta::minutes(1));
                health.control_succeeded(at);
                health.scheduler_succeeded(at);
            },
            expected_status: HealthStatus::Healthy,
            expected_components: vec![
                ("Server", HealthStatus::Healthy, None),
                ("Control database", HealthStatus::Healthy, None),
                ("Scheduler", HealthStatus::Healthy, None),
            ],
        },
        ReportTestCase {
            description: "a location that stopped reporting",
            events: |health, at| {
                health.control_succeeded(at);
                health.scheduler_succeeded(at);
                locations(health, at);
            },
            expected_status: HealthStatus::Degraded,
            expected_components: vec![
                ("Server", HealthStatus::Healthy, None),
                ("Control database", HealthStatus::Healthy, None),
                ("Scheduler", HealthStatus::Healthy, None),
                ("orders", HealthStatus::Healthy, None),
                (
                    "finance",
                    HealthStatus::Degraded,
                    Some("Hasn't reported since 2026-11-25 13:45 UTC"),
                ),
            ],
        },
    ];
    let at: DateTime<Utc> = "2026-11-25T14:05:00Z".parse().expect("time");
    for test_case in test_cases {
        let health = ServerHealth::new("1.0.0", true);
        (test_case.events)(&health, at);
        let report = health_report(&health, at);
        let components: Vec<(String, HealthStatus, Option<String>)> = report
            .components
            .into_iter()
            .map(|component| (component.name, component.status, component.reason))
            .collect();
        let expected: Vec<(String, HealthStatus, Option<String>)> = test_case
            .expected_components
            .into_iter()
            .map(|(name, status, reason)| (name.to_owned(), status, reason.map(str::to_owned)))
            .collect();
        assert_eq!(
            (report.status, components),
            (test_case.expected_status, expected),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_failing_database_when_it_keeps_failing_then_since_stays_at_the_first_failure() {
    let health = ServerHealth::new("1.0.0", false);
    let first: DateTime<Utc> = "2026-11-25T14:05:00Z".parse().expect("time");
    health.control_succeeded(first - TimeDelta::minutes(1));
    unresolved(&health, first);
    unresolved(&health, first + TimeDelta::minutes(30));
    assert_eq!(health.control_failing_since(), Some(first));
}

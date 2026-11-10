use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::api::tests::test_types::UpcomingTestCase;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_person_triggers::list_person_triggers;
use crate::scheduler::tests::contracts::ticks::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_running_and_paused_schedules_when_people_list_triggers_then_running_ones_carry_their_next_due_times()
 {
    let hourly = [
        "2031-01-01T01:00:00Z",
        "2031-01-01T02:00:00Z",
        "2031-01-01T03:00:00Z",
        "2031-01-01T04:00:00Z",
        "2031-01-01T05:00:00Z",
    ];
    let calendar = [
        "2031-02-01T00:00:00Z",
        "2031-03-01T00:00:00Z",
        "2031-04-01T00:00:00Z",
    ];
    let test_cases = [
        UpcomingTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_hourly: hourly,
            expected_calendar: calendar,
        },
        UpcomingTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_hourly: hourly,
            expected_calendar: calendar,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.documents.insert(
                "triggers".into(),
                json!({
                    "hourly_orders": {"description": "Hourly orders", "job_key": "orders",
                        "starts_at": "2031-01-01T00:30:00Z",
                        "schedule": {"kind": "cron", "expression": "0 * * * *", "timezone": "UTC"}},
                    "paused_orders": {"description": "Paused orders", "job_key": "orders",
                        "default_status": "paused", "starts_at": "2031-01-01T00:30:00Z",
                        "schedule": {"kind": "cron", "expression": "0 * * * *", "timezone": "UTC"}},
                    "month_start_orders": {"description": "Month start orders", "job_key": "orders",
                        "schedule": {"kind": "calendar", "times": test_case.expected_calendar,
                            "refresh_at": "2031-01-15T00:00:00Z"}}
                }),
            );
            publish_snapshot(store, &input).await.expect("publication");
            let audit = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let administrator = bootstrap_user(store, &audit, "orders-admin")
                .await
                .expect("administrator");
            let listed: BTreeMap<String, Vec<DateTime<Utc>>> = list_person_triggers(
                store,
                (&audit, administrator),
                (location, "development"),
                DefaultRole::None,
            )
            .await
            .expect("person triggers")
            .into_iter()
            .map(|record| (record.trigger.name, record.upcoming))
            .collect();
            let times = |texts: &[&str]| -> Vec<DateTime<Utc>> {
                texts
                    .iter()
                    .map(|text| text.parse().expect("time"))
                    .collect()
            };
            assert_eq!(
                listed,
                BTreeMap::from([
                    (
                        "hourly_orders".to_owned(),
                        times(&test_case.expected_hourly)
                    ),
                    (
                        "month_start_orders".to_owned(),
                        times(&test_case.expected_calendar)
                    ),
                    ("paused_orders".to_owned(), Vec::new()),
                ]),
                "{}: five cron times from the start, the calendar's remaining dates, none while paused",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

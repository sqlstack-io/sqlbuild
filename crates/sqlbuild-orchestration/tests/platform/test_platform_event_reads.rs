use chrono::TimeDelta;
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::api::tests::helpers::stop_api;
use crate::api::tests::warehouse_history_helpers::{get, login, seed_target, start_password_api};
use crate::platform::_helpers::health::observations::observe;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::models::{
    LocationState, PersonPlatformEventPage, PlatformEvent, PlatformEventKind, PlatformEventPayload,
    PlatformProblems, WarehouseReadFailure, WarehouseSyncFailure,
};
use crate::platform::tests::helpers::{snapshot, sync};
use crate::platform::tests::test_types::PlatformEventReadTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlEvent, Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// Every kind a person reads from stored events, in the order `PlatformEventKind` sorts.
const EVERY_READABLE_KIND: [PlatformEventKind; 8] = [
    PlatformEventKind::SchedulerNoLeader,
    PlatformEventKind::TickOverdue,
    PlatformEventKind::CalendarHorizonLow,
    PlatformEventKind::LocationBroken,
    PlatformEventKind::LocationFailing,
    PlatformEventKind::TriggerSkipped,
    PlatformEventKind::SensorRechecked,
    PlatformEventKind::CalendarTimesDropped,
];

#[test]
fn given_every_stored_event_kind_and_a_failing_sync_when_a_person_reads_problems_then_each_is_listed()
 {
    let test_cases = [
        PlatformEventReadTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_kinds: EVERY_READABLE_KIND,
            expected_unreadable_type: "platform_cost_spike",
        },
        PlatformEventReadTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_kinds: EVERY_READABLE_KIND,
            expected_unreadable_type: "platform_cost_spike",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let (location_id, session_id) = seed_target(store).await;
            let mut triggered = snapshot(location_id);
            triggered.generation = 2;
            publish_snapshot(store, &triggered)
                .await
                .expect("snapshot with triggers");
            let mut failing = sync(
                0,
                Some(WarehouseSyncFailure::Read),
                (Some(WarehouseReadFailure::Unreachable {}), false),
            )
            .report;
            (failing.session_id, failing.target) = (session_id, "development".into());
            report_location_health(store, location_id, &failing)
                .await
                .expect("failing sync report");
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("observe events");
            let (mut transaction, rows) = query(
                transaction,
                "SELECT trigger_id FROM sqb_triggers WHERE location_id=$1 LIMIT 1",
                vec![Parameter::Id(location_id)],
            )
            .await
            .expect("a trigger");
            let trigger_id: Uuid =
                serde_json::from_value(rows[0]["trigger_id"].clone()).expect("trigger ID");
            let (now, snapshot_id) = (transaction.now, Uuid::now_v7());
            for event in [
                PlatformEvent::SchedulerNoLeader,
                PlatformEvent::LocationBroken {
                    location_id,
                    target: "development".into(),
                    last_seen_at: now,
                },
                PlatformEvent::LocationFailing {
                    location_id,
                    target: "development".into(),
                    state: LocationState::Degraded,
                    failed_at: now,
                    error: Some("IP is not allowed to access the warehouse".into()),
                    serving_generation: Some(1),
                },
                PlatformEvent::TickOverdue {
                    trigger_id,
                    scheduled_for: now,
                },
                PlatformEvent::CalendarHorizonLow {
                    trigger_id,
                    refresh_at: now,
                    horizon_end: None,
                },
                PlatformEvent::TriggerSkipped {
                    trigger_id,
                    snapshot_id,
                    reason: "its pinned snapshot can't be read".into(),
                },
                PlatformEvent::SensorRechecked {
                    trigger_id,
                    snapshot_id,
                    evaluation_id: Uuid::now_v7(),
                },
                PlatformEvent::CalendarTimesDropped {
                    trigger_id,
                    snapshot_id,
                    times: vec![now],
                },
            ] {
                transaction = observe(transaction, Uuid::nil(), (event, Some(now)))
                    .await
                    .expect("observe");
            }
            let newer = Uuid::now_v7();
            let earlier_run_failure = json!({"platform": {"kind": "run_failed", "run_id": Uuid::now_v7(),
                "run_kind": "job", "job_key": "orders", "location_id": location_id,
                "target": "development", "outcome": "failed", "failed_nodes": [], "failed_node_count": 0}});
            assert!(
                serde_json::from_value::<PlatformEventPayload>(earlier_run_failure.clone())
                    .expect_err("the run failure shape 0.261.0 stored")
                    .to_string()
                    .contains("missing field `trigger`"),
                "the stored run failure is the one 0.262.0 couldn't read"
            );
            for (event_id, event_type, payload) in [
                (Uuid::now_v7(), "platform_run_failed", earlier_run_failure),
                (
                    newer,
                    test_case.expected_unreadable_type,
                    json!({"platform": {"kind": "cost_spike", "location_id": location_id,
                        "target": "development", "credits": 12}}),
                ),
            ] {
                transaction = append_control_event(
                    transaction,
                    ControlEvent {
                        event_id,
                        invocation_id: format!("platform/{}/stored", Uuid::nil()),
                        event_type,
                        producer: "platform",
                        occurred_at: now - TimeDelta::seconds(1),
                        run_id: None,
                        payload,
                    },
                )
                .await
                .expect("an event another release stored")
                .0;
            }
            commit(transaction).await.expect("observed");

            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let (status, events) = get(
                &client,
                (&api.url, &viewer),
                "/api/control/platform/events?limit=200",
            )
            .await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}: {events}",
                test_case.description
            );
            let page: PersonPlatformEventPage =
                serde_json::from_value(events).expect("the page the UI's schema describes");
            let mut kinds: Vec<PlatformEventKind> =
                page.events.iter().map(|event| event.event.kind()).collect();
            kinds.sort();
            assert_eq!(
                kinds, test_case.expected_kinds,
                "{}: every readable kind is listed, and an older release's run failure never fails the read",
                test_case.description
            );
            assert_eq!(
                page.unreadable
                    .iter()
                    .map(|item| (
                        item.event_id,
                        item.event_type.as_str(),
                        item.location_id,
                        item.target.as_deref()
                    ))
                    .collect::<Vec<_>>(),
                [(
                    newer,
                    test_case.expected_unreadable_type,
                    Some(location_id),
                    Some("development")
                )],
                "{}: a kind this server can't read is listed on its own, scoped to its target",
                test_case.description
            );
            assert!(
                page.unreadable[0].error.contains("cost_spike"),
                "the reason names what couldn't be read: {}",
                page.unreadable[0].error
            );

            let (status, problems) = get(
                &client,
                (&api.url, &viewer),
                "/api/control/platform/problems",
            )
            .await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}: {problems}",
                test_case.description
            );
            let problems: PlatformProblems =
                serde_json::from_value(problems).expect("the problems the UI's schema describes");
            assert_eq!(
                problems
                    .failing_syncs
                    .iter()
                    .map(|sync| (sync.location_id, sync.unreachable))
                    .collect::<Vec<_>>(),
                [(location_id, true)],
                "{}: the failing sync is a problem of its own",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

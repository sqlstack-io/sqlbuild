use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::acknowledgements::main::dismiss_person_skipped_event::dismiss_person_skipped_event;
use crate::acknowledgements::models::SkippedEventDismissalRequest;
use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::person_reads::list_person_platform_events::list_person_platform_events;
use crate::platform::main::person_reads::read_person_problems::read_person_problems;
use crate::platform::models::PlatformEvent;
use crate::platform::tests::helpers::skip_unreadable;
use crate::platform::tests::test_types::SkippedEventDismissalTestCase;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlEvent, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_skipped_unreadable_event_when_a_person_dismisses_it_then_it_leaves_problems_for_good_and_stays_in_history()
 {
    let test_cases = [
        SkippedEventDismissalTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_type: "platform_run_failed",
        },
        SkippedEventDismissalTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_type: "platform_run_failed",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            // A run failure as 0.261.0 stored it, without the `trigger` this release requires.
            let unreadable = Uuid::now_v7();
            let (transaction, _) = append_control_event(
                begin(store, TransactionMode::Write).await.expect("write"),
                ControlEvent {
                    event_id: unreadable,
                    invocation_id: "platform/test-health/stored".into(),
                    event_type: "platform_run_failed",
                    producer: "platform",
                    occurred_at: Utc::now(),
                    run_id: None,
                    payload: json!({"platform": {"kind": "run_failed", "run_id": Uuid::now_v7(),
                        "run_kind": "job", "job_key": "orders", "location_id": location,
                        "target": "development", "outcome": "failed", "failed_nodes": [],
                        "failed_node_count": 0}}),
                },
            )
            .await
            .expect("an event an earlier release stored");
            commit(transaction).await.expect("stored");
            skip_unreadable(store).await;
            let skipped = || async {
                read_person_problems(store, admin, DefaultRole::None)
                    .await
                    .expect("problems")
                    .skipped_events
            };
            let listed = skipped().await;
            assert_eq!(
                listed
                    .iter()
                    .map(|event| (event.event_id, event.event_type.as_str()))
                    .collect::<Vec<_>>(),
                [(unreadable, test_case.expected_type)],
                "{}: the skip is a problem",
                test_case.description
            );
            let request = SkippedEventDismissalRequest {
                event_id: Uuid::now_v7(),
                storage_id: listed[0].storage_id,
            };
            let context = &context;
            let dismiss = |target: &'static str, request: SkippedEventDismissalRequest| async move {
                dismiss_person_skipped_event(
                    store,
                    (context, admin),
                    (location, target, &request),
                    DefaultRole::None,
                )
                .await
            };
            assert!(
                dismiss("staging", request.clone()).await.is_err(),
                "{}: another target's view can't dismiss it",
                test_case.description
            );
            assert_eq!(skipped().await.len(), 1, "{}", test_case.description);
            assert!(
                dismiss("development", request.clone())
                    .await
                    .expect("dismiss"),
                "{}: dismissing applies once",
                test_case.description
            );
            assert!(
                skipped().await.is_empty(),
                "{}: a dismissed skip leaves the problems",
                test_case.description
            );
            // Readers meet the same stored event again; a retry and a second dismissal apply nothing.
            skip_unreadable(store).await;
            assert!(
                skipped().await.is_empty(),
                "{}: skipping the same stored event again doesn't bring it back",
                test_case.description
            );
            assert!(
                !dismiss("development", request.clone())
                    .await
                    .expect("retry"),
                "{}",
                test_case.description
            );
            let again = SkippedEventDismissalRequest {
                event_id: Uuid::now_v7(),
                ..request
            };
            assert!(
                !dismiss("development", again).await.expect("dismiss again"),
                "{}",
                test_case.description
            );
            let history = list_person_platform_events(store, admin, (None, 200), DefaultRole::None)
                .await
                .expect("history");
            assert_eq!(
                history
                    .events
                    .iter()
                    .filter(|event| matches!(
                        &event.event,
                        PlatformEvent::UnreadableEventSkipped { event_id, .. } if *event_id == unreadable
                    ))
                    .count(),
                1,
                "{}: the skip stays in the history, once",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

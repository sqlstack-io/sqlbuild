//! A location's health reports reach the pages people watch as they arrive, never the location itself.

use chrono::Utc;
use uuid::Uuid;

use crate::api::tests::changes_helpers::{connect, next_topic, opened, request_person_feed};
use crate::api::tests::helpers::{stop_api, sync_report};
use crate::api::tests::test_types::LocationHealthFeedTestCase;
use crate::api::tests::warehouse_history_helpers::{login, seed_target, start_password_api};
use crate::credentials::main::tokens::issue_token::issue_token;
use crate::credentials::models::{AuditContext, CredentialScope};
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport, WarehouseSyncFailure};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_page_following_a_target_when_its_location_reports_health_then_the_page_hears_it_and_the_location_does_not()
 {
    let test_cases = [
        LocationHealthFeedTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_topics: ["warehouse_sync", "warehouse_sync", "location_health"],
        },
        LocationHealthFeedTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_topics: ["warehouse_sync", "warehouse_sync", "location_health"],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let (location_id, session_id) = seed_target(store).await;
            let token = issue_token(
                store,
                &AuditContext {
                    actor: "test-admin".into(),
                    granting_role: "admin".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                },
                CredentialScope::Location { location_id },
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .expect("location token");
            let (api, client) = start_password_api(store).await;
            let cookie = login(&client, &api.url, "orders-viewer").await;
            let session = cookie
                .strip_prefix("__Host-sqb-session=")
                .expect("session cookie");
            let mut page = opened(
                request_person_feed(&client, &api, (location_id, "development", session)).await,
            )
            .await;
            let mut location = connect(
                &client,
                &api,
                (location_id, "development", token.bearer.as_str()),
            )
            .await;
            for (index, (report, what)) in [
                (
                    sync_report(session_id, (Some(WarehouseSyncFailure::Read), false)),
                    "a failing sync",
                ),
                (sync_report(session_id, (None, true)), "the sync completing"),
                (
                    LocationHealthReport {
                        event_id: Uuid::now_v7(),
                        session_id: Uuid::now_v7(),
                        target: "development".into(),
                        health: LocationHealth::Heartbeat {},
                    },
                    "a check-in",
                ),
            ]
            .into_iter()
            .enumerate()
            {
                assert!(
                    report_location_health(store, location_id, &report)
                        .await
                        .expect("health report"),
                    "{}: {what} is recorded",
                    test_case.description
                );
                assert_eq!(
                    next_topic(&mut page).await.as_deref(),
                    Some(format!("{}:{location_id}", test_case.expected_topics[index]).as_str()),
                    "{}: {what} after the page loaded reaches it without a reload",
                    test_case.description
                );
            }
            let mut next = snapshot(location_id);
            next.generation = 2;
            publish_snapshot(store, &next).await.expect("next snapshot");
            assert_eq!(
                next_topic(&mut location).await,
                Some(format!("snapshot:{location_id}")),
                "{}: the location hears its next snapshot, not its own health",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

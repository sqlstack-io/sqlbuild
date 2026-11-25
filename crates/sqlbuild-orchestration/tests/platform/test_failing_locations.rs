//! A checking-in location whose latest load failed is a problem people see and a platform event alerts receive, once.

use uuid::Uuid;

use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::_helpers::location::liveness::observe_failing_locations;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::main::person_reads::read_person_problems::read_person_problems;
use crate::platform::models::{LocationHealth, LocationHealthReport, LocationState};
use crate::platform::tests::test_types::FailingLocationTestCase;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::query::query;
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_location_whose_latest_load_failed_when_problems_are_read_then_it_is_a_problem_and_one_event()
 {
    let test_cases = [
        FailingLocationTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_error: "job loaders_job: path selectors require an explicit root",
        },
        FailingLocationTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_error: "job loaders_job: path selectors require an explicit root",
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
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let session_id = Uuid::now_v7();
            for health in [
                LocationHealth::Heartbeat {},
                LocationHealth::CompileFailed {
                    generation: 2,
                    rules: None,
                    error: Some(test_case.expected_error.to_owned()),
                },
            ] {
                report_location_health(
                    store,
                    location,
                    &LocationHealthReport {
                        event_id: Uuid::now_v7(),
                        session_id,
                        target: "development".into(),
                        health,
                    },
                )
                .await
                .expect("report");
            }
            let problems = read_person_problems(store, admin, DefaultRole::None)
                .await
                .expect("problems");
            let failing: Vec<(LocationState, Option<String>, Option<i64>)> = problems
                .failing_locations
                .iter()
                .map(|failing| {
                    (
                        failing.state,
                        failing.error.clone(),
                        failing.serving.as_ref().map(|snapshot| snapshot.generation),
                    )
                })
                .collect();
            assert_eq!(
                failing,
                [(
                    LocationState::FallingBack,
                    Some(test_case.expected_error.to_owned()),
                    Some(published.generation)
                )],
                "{}: the failure, its error and the generation served instead",
                test_case.description
            );
            let installation = {
                let transaction = begin(store, TransactionMode::Read).await.expect("begin");
                let (transaction, rows) = query(
                    transaction,
                    "SELECT installation_id FROM sqb_installation",
                    Vec::new(),
                )
                .await
                .expect("installation");
                commit(transaction).await.expect("commit");
                rows[0]["installation_id"]
                    .as_str()
                    .and_then(|id| id.parse::<Uuid>().ok())
                    .expect("an installation")
            };
            observe_failing_locations(store, installation)
                .await
                .expect("scan");
            observe_failing_locations(store, installation)
                .await
                .expect("scan again");
            let transaction = begin(store, TransactionMode::Read).await.expect("begin");
            let (transaction, events) = query(
                transaction,
                "SELECT payload FROM recall_events WHERE event_type=$1",
                vec![Parameter::Text("platform_location_failing".into())],
            )
            .await
            .expect("events");
            commit(transaction).await.expect("commit");
            let states: Vec<&str> = events
                .iter()
                .filter_map(|row| row["payload"]["platform"]["state"].as_str())
                .collect();
            assert_eq!(
                states,
                ["falling_back"],
                "{}: one platform event per failure, however often the watchdog scans",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

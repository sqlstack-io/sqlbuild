use std::collections::BTreeMap;

use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::platform::models::PlatformEvent;
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::models::{NodeOutcome, RunChange, RunFailurePhase};
use crate::runs::tests::failure_event_helpers::{MISSING, end_run, report, summarize};
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::runs::tests::test_types::{RunFailureEventTestCase, RunFailureSummary};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::helpers::read_recall_events;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_failed_run_when_it_finishes_then_its_run_failed_event_says_why_where_and_links_to_it() {
    let test_cases = [
        RunFailureEventTestCase {
            description: "SQLite: a model failing in its post-hook names the phase and links to the node and its output",
            backend: Backend::Sqlite,
            nodes: &[
                ("customers", NodeOutcome::Success, None),
                ("orders", NodeOutcome::Failed, Some("post_hook")),
            ],
            failed_end_audits: &[],
            expected_summary: RunFailureSummary {
                links_to_run: true,
                first_node: Some((
                    Some("post_hook".into()),
                    Some("?node=orders".into()),
                    Some("/output/orders".into()),
                )),
                first_audit: None,
                failed_audit_count: 0,
                succeeded_node_count: 1,
                error: None,
            },
        },
        RunFailureEventTestCase {
            description: "SQLite: a run failed only by an end-of-build audit names it, with the warehouse's error",
            backend: Backend::Sqlite,
            nodes: &[
                ("customers", NodeOutcome::Success, None),
                ("orders", NodeOutcome::Success, None),
            ],
            failed_end_audits: &["orders_match_payments"],
            expected_summary: RunFailureSummary {
                links_to_run: true,
                first_node: None,
                first_audit: Some((
                    "orders_match_payments".into(),
                    Some("42S02".into()),
                    Some(MISSING.into()),
                )),
                failed_audit_count: 1,
                succeeded_node_count: 2,
                error: None,
            },
        },
        RunFailureEventTestCase {
            description: "SQLite: a run that failed before any node says where and why",
            backend: Backend::Sqlite,
            nodes: &[],
            failed_end_audits: &[],
            expected_summary: RunFailureSummary {
                links_to_run: true,
                first_node: None,
                first_audit: None,
                failed_audit_count: 0,
                succeeded_node_count: 0,
                error: Some((
                    RunFailurePhase::Load,
                    Some("project could not be loaded".into()),
                )),
            },
        },
        RunFailureEventTestCase {
            description: "PostgreSQL: a model failing in its post-hook names the phase and links to the node and its output",
            backend: Backend::Postgres,
            nodes: &[
                ("customers", NodeOutcome::Success, None),
                ("orders", NodeOutcome::Failed, Some("post_hook")),
            ],
            failed_end_audits: &[],
            expected_summary: RunFailureSummary {
                links_to_run: true,
                first_node: Some((
                    Some("post_hook".into()),
                    Some("?node=orders".into()),
                    Some("/output/orders".into()),
                )),
                first_audit: None,
                failed_audit_count: 0,
                succeeded_node_count: 1,
                error: None,
            },
        },
        RunFailureEventTestCase {
            description: "PostgreSQL: a run failed only by an end-of-build audit names it, with the warehouse's error",
            backend: Backend::Postgres,
            nodes: &[
                ("customers", NodeOutcome::Success, None),
                ("orders", NodeOutcome::Success, None),
            ],
            failed_end_audits: &["orders_match_payments"],
            expected_summary: RunFailureSummary {
                links_to_run: true,
                first_node: None,
                first_audit: Some((
                    "orders_match_payments".into(),
                    Some("42S02".into()),
                    Some(MISSING.into()),
                )),
                failed_audit_count: 1,
                succeeded_node_count: 2,
                error: None,
            },
        },
        RunFailureEventTestCase {
            description: "PostgreSQL: a run that failed before any node says where and why",
            backend: Backend::Postgres,
            nodes: &[],
            failed_end_audits: &[],
            expected_summary: RunFailureSummary {
                links_to_run: true,
                first_node: None,
                first_audit: None,
                failed_audit_count: 0,
                succeeded_node_count: 0,
                error: Some((
                    RunFailurePhase::Load,
                    Some("project could not be loaded".into()),
                )),
            },
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
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            queue(
                store,
                &request(
                    (location, published.snapshot_id),
                    1,
                    1,
                    &["customers", "orders"],
                ),
            )
            .await;
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("available");
            report(store, &claim, RunChange::Started).await;
            end_run(
                store,
                &claim,
                (test_case.nodes, test_case.failed_end_audits),
            )
            .await;
            let events: Vec<PlatformEvent> = read_recall_events(store, 0, 200)
                .await
                .expect("facts")
                .into_iter()
                .filter(|row| row["event_type"] == "platform_run_failed")
                .map(|row| {
                    serde_json::from_value(row["payload"]["platform"].clone())
                        .expect("typed run failure")
                })
                .collect();
            let [PlatformEvent::RunFailed(failure)] = events.as_slice() else {
                panic!(
                    "{}: one run_failed event, got {events:?}",
                    test_case.description
                );
            };
            assert_eq!(
                summarize(failure),
                test_case.expected_summary,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

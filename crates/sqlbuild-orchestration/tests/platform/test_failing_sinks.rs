//! A sink is failing while its newest run lost events, since the first lossy run in a row; a clean run clears it.

use uuid::Uuid;

use crate::platform::_helpers::health::failing_sinks::{failing_sinks, sink_reports};
use crate::platform::models::FailingSink;
use crate::platform::tests::helpers::{delivered, failing_sink_of, report};
use crate::platform::tests::test_types::{FailingSinkTestCase, StoredSinkTestCase};
use crate::run_output::tests::helpers::claimed_run;
use crate::runs::main::execution::change_run::change_run;
use crate::runs::models::{RunAuthority, RunChange, RunChangeRequest};
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_sink_reports_when_scanning_then_a_sink_fails_from_its_first_lossy_run_in_a_row() {
    let test_cases = [
        FailingSinkTestCase {
            description: "two lossy runs after a clean one, newest first",
            reports: vec![
                report(3, vec![delivered("kafka", (8, 0, 8))], vec![]),
                report(2, vec![delivered("kafka", (5, 1, 0))], vec![]),
                report(1, vec![delivered("kafka", (4, 0, 0))], vec![]),
            ],
            expected_sinks: vec![FailingSink {
                since: crate::platform::tests::helpers::at(2),
                last_success_at: Some(crate::platform::tests::helpers::at(1)),
                ..failing_sink_of("kafka", "8 of 8 events not delivered (8 failed)")
            }],
        },
        FailingSinkTestCase {
            description: "a clean newest run clears it",
            reports: vec![
                report(3, vec![delivered("kafka", (8, 0, 0))], vec![]),
                report(2, vec![delivered("kafka", (8, 0, 8))], vec![]),
            ],
            expected_sinks: vec![],
        },
        FailingSinkTestCase {
            description: "a provider whose teardown failed may have lost what it held",
            reports: vec![report(
                3,
                vec![delivered("kafka", (8, 0, 0))],
                vec!["'kafka' (KafkaProducer) raised KafkaException".into()],
            )],
            expected_sinks: vec![failing_sink_of(
                "sink providers",
                "closing failed, so events they held may be lost: 'kafka' (KafkaProducer) raised KafkaException",
            )],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            failing_sinks(&test_case.reports),
            test_case.expected_sinks,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_run_whose_sink_failed_when_the_store_is_scanned_then_the_sink_is_failing() {
    let test_cases = [
        StoredSinkTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_sinks: vec![(
                "kafka".into(),
                "8 of 8 events not delivered (8 failed)".into(),
            )],
        },
        StoredSinkTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_sinks: vec![(
                "kafka".into(),
                "8 of 8 events not delivered (8 failed)".into(),
            )],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let (_, claim) = claimed_run(store).await;
            for change in [
                RunChange::Started,
                RunChange::SinkDeliveries {
                    sinks: vec![delivered("kafka", (8, 0, 8))],
                    teardown_failures: vec![],
                },
            ] {
                change_run(
                    store,
                    RunAuthority::Worker {
                        run_id: claim.request.run_id,
                        claim_id: claim.claim_id,
                    },
                    &RunChangeRequest {
                        event_id: Uuid::now_v7(),
                        run_id: claim.request.run_id,
                        claim_id: Some(claim.claim_id),
                        change,
                    },
                )
                .await
                .expect("worker fact");
            }
            let transaction = begin(store, TransactionMode::Read).await.expect("read");
            let (transaction, reports) = sink_reports(transaction).await.expect("sink reports");
            commit(transaction).await.expect("commit");
            let failing: Vec<(String, String)> = failing_sinks(&reports)
                .into_iter()
                .map(|sink| (sink.sink, sink.reason))
                .collect();
            assert_eq!(
                failing, test_case.expected_sinks,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

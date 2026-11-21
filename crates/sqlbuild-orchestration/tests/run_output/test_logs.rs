//! Lines a node logs with `ctx.log` are kept with the run and read back with its output index.

use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::run_output::main::read_person_output_index::read_person_output_index;
use crate::run_output::tests::helpers::{claimed_run, reader};
use crate::run_output::tests::test_types::NodeLogTestCase;
use crate::runs::main::execution::change_run::change_run;
use crate::runs::models::{NodeLogLevel, RunAuthority, RunChange, RunChangeRequest};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_worker_reporting_ctx_log_lines_when_the_output_index_is_read_then_they_read_back_with_their_levels()
 {
    let test_cases = [
        NodeLogTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_logs: vec![
                ("model:orders", "info", "loaded 3 orders"),
                ("model:orders", "warning", "2 orders had no customer"),
            ],
        },
        NodeLogTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_logs: vec![
                ("model:orders", "info", "loaded 3 orders"),
                ("model:orders", "warning", "2 orders had no customer"),
            ],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let (location, claim) = claimed_run(store).await;
            let worker = RunAuthority::Worker {
                run_id: claim.request.run_id,
                claim_id: claim.claim_id,
            };
            let report = |change| RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: claim.request.run_id,
                claim_id: Some(claim.claim_id),
                change,
            };
            let log = |level, message: &str| RunChange::NodeLog {
                node_key: "model:orders".into(),
                level,
                message: message.into(),
            };
            for change in [
                RunChange::Started,
                log(NodeLogLevel::Info, "loaded 3 orders"),
                log(NodeLogLevel::Warning, "2 orders had no customer"),
            ] {
                change_run(store, worker, &report(change))
                    .await
                    .expect("worker fact");
            }
            let oversized = change_run(
                store,
                worker,
                &report(log(NodeLogLevel::Error, &"x".repeat(5000))),
            )
            .await;
            assert!(
                oversized.is_err(),
                "{}: an oversized message is refused",
                test_case.description
            );
            let (context, admin) = reader(store).await;
            let index = read_person_output_index(
                store,
                (
                    (&context, admin),
                    (location, "development", claim.request.run_id),
                    DefaultRole::None,
                ),
                None,
            )
            .await
            .expect("index")
            .expect("run");
            let logs: Vec<(&str, &str, &str)> = index
                .logs
                .iter()
                .map(|line| {
                    (
                        line.node_key.as_str(),
                        line.level.as_str(),
                        line.message.as_str(),
                    )
                })
                .collect();
            assert_eq!(logs, test_case.expected_logs, "{}", test_case.description);
            close_test_store(fixture).await;
        });
    }
}

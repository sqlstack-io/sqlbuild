//! A run's node list says what each node did: its materialization, the rows it added or the table now holds, its progress, why it changed nothing, and which nodes never ran.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::execution::change_run::change_run;
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::main::listings::list_run_nodes::list_run_nodes;
use crate::runs::models::{NodeOutcome, RunAuthority, RunChange, RunChangeRequest};
use crate::runs::tests::helpers::{queue, request, rows, snapshot};
use crate::runs::tests::test_types::NodeResultsTestCase;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const EXPECTED_NODES: &[crate::runs::tests::test_types::NodeResultRow] = &[
    (
        "customers",
        "success",
        Some("incremental"),
        (Some(48), None, None),
        None,
    ),
    (
        "inventory",
        "not_run",
        Some("table"),
        (None, None, None),
        None,
    ),
    (
        "orders",
        "success",
        Some("table"),
        (None, Some(500), Some(120)),
        None,
    ),
    (
        "products",
        "success",
        Some("view"),
        (None, None, None),
        Some("unchanged since its last build"),
    ),
];

#[test]
fn given_a_run_reporting_what_each_node_did_when_its_nodes_are_listed_then_rows_progress_no_change_and_not_run_read_back()
 {
    let test_cases = [
        NodeResultsTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_nodes: EXPECTED_NODES,
            expected_not_run: 1,
        },
        NodeResultsTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_nodes: EXPECTED_NODES,
            expected_not_run: 1,
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
            for (key, materialization) in [
                ("customers", "incremental"),
                ("inventory", "table"),
                ("orders", "table"),
                ("products", "view"),
            ] {
                input.nodes.get_mut(key).expect("node").definition =
                    json!({"materialization": materialization});
            }
            let published = publish_snapshot(store, &input).await.expect("publish");
            let pinned = request(
                (location, published.snapshot_id),
                41,
                0,
                &["customers", "inventory", "orders", "products"],
            );
            queue(store, &pinned).await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("claimable");
            let finished =
                |node: &str, outcome, (rows_affected, table_rows), reason: Option<&str>| {
                    RunChange::NodeFinished {
                        node_key: node.into(),
                        outcome,
                        rows_affected,
                        table_rows,
                        no_change_reason: reason.map(str::to_owned),
                        error_code: None,
                        skipped_because_of: None,
                        error_message: None,
                    }
                };
            for change in [
                RunChange::Started,
                RunChange::NodeStarted {
                    node_key: "orders".into(),
                },
                RunChange::NodeProgress {
                    node_key: "orders".into(),
                    rows_so_far: 120,
                },
                finished("orders", NodeOutcome::Success, (None, Some(500)), None),
                finished("customers", NodeOutcome::Success, (Some(48), None), None),
                finished(
                    "products",
                    NodeOutcome::Success,
                    (None, None),
                    Some("unchanged since its last build"),
                ),
                finished("inventory", NodeOutcome::NotRun, (None, None), None),
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
            let nodes: Vec<_> = list_run_nodes(store, claim.request.run_id)
                .await
                .expect("nodes")
                .into_iter()
                .map(|node| {
                    (
                        node.node_key,
                        node.status,
                        node.materialization,
                        (node.rows_affected, node.table_rows, node.rows_so_far),
                        node.no_change_reason,
                    )
                })
                .collect();
            let expected: Vec<_> = test_case
                .expected_nodes
                .iter()
                .map(|(key, status, materialization, counts, reason)| {
                    (
                        (*key).to_owned(),
                        (*status).to_owned(),
                        materialization.map(str::to_owned),
                        *counts,
                        reason.map(str::to_owned),
                    )
                })
                .collect();
            assert_eq!(nodes, expected, "{}", test_case.description);
            let counts = rows(
                store,
                "SELECT node_counts_json FROM sqb_runs WHERE run_id=$1",
                vec![Parameter::Id(claim.request.run_id)],
            )
            .await;
            assert_eq!(
                crate::runs::tests::helpers::json_column(&counts[0]["node_counts_json"])["not_run"],
                json!(test_case.expected_not_run),
                "{}: the run counts nodes it never reached apart from skips",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

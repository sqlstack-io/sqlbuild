//! A failed node's error text is redacted and bounded once a worker reports it, and only evidence on that node reads it.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::role_helpers::{credentials, orders_grant, role_fixture};
use crate::runs::main::change_run::change_run;
use crate::runs::main::claim_next_run::claim_next_run;
use crate::runs::main::read_person_run_node_error::read_person_run_node_error;
use crate::runs::models::{RunAuthority, RunChangeRequest};
use crate::runs::tests::helpers::{queue, request, rows, snapshot};
use crate::runs::tests::test_types::RunNodeErrorTestCase;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const EXPECTED_MESSAGE: &str = "Conversion Error: could not convert 'n/a' to INT32 \
     (password=[REDACTED], url=postgres://orders:[REDACTED]@warehouse.example.test/orders, \
     cut 'postgres://orders:[REDACTED] by the warehouse, port http://warehouse.example.test:8080/health) ";

#[test]
fn given_a_failed_node_when_people_read_its_error_then_only_evidence_on_that_node_sees_redacted_bounded_text()
 {
    let test_cases = [
        RunNodeErrorTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_message: EXPECTED_MESSAGE,
        },
        RunNodeErrorTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_message: EXPECTED_MESSAGE,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let mut input = snapshot(roles.location_id);
            input.generation = 2;
            input.documents.insert(
                "roles".into(),
                json!({"orders_support": {"description": "Answer order questions",
                    "grants": [orders_grant(json!(["view", "view.evidence"]), &["orders"])]}}),
            );
            input
                .nodes
                .get_mut("orders")
                .expect("orders node")
                .definition = json!({"materialization": "table", "path": "models\\orders.sql"});
            let published = publish_snapshot(store, &input).await.expect("publish");
            queue(
                store,
                &request(
                    (roles.location_id, published.snapshot_id),
                    1,
                    1,
                    &["customers", "orders"],
                ),
            )
            .await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("queued run");
            let run_id = claim.request.run_id;
            let raw = format!(
                "Conversion Error: could not convert 'n/a' to INT32 (password=orders-db-pass, \
                 url=postgres://orders:orders-db-pass@warehouse.example.test/orders, \
                 cut 'postgres://orders:orders-d... by the warehouse, port http://warehouse.example.test:8080/health) {}",
                "x".repeat(3000)
            );
            for change in [
                json!({"kind": "started"}),
                json!({"kind": "node_finished", "node_key": "orders", "outcome": "failed",
                    "rows_affected": null, "error_code": "T002", "skipped_because_of": null,
                    "error_message": raw}),
                json!({"kind": "node_finished", "node_key": "customers", "outcome": "skipped",
                    "rows_affected": null, "error_code": null, "skipped_because_of": "orders"}),
            ] {
                let reported: RunChangeRequest = serde_json::from_value(json!({
                    "event_id": Uuid::now_v7(), "run_id": run_id,
                    "claim_id": claim.claim_id, "change": change,
                }))
                .expect("a worker's report body");
                change_run(
                    store,
                    RunAuthority::Worker {
                        run_id,
                        claim_id: claim.claim_id,
                    },
                    &reported,
                )
                .await
                .expect("worker report");
            }
            let member = (&roles.context, roles.member_id);
            let location = roles.location_id;
            let read = |who, run_id, node: &'static str| {
                read_person_run_node_error(store, who, (location, "development", run_id, node))
            };
            let error = read(member, run_id, "orders")
                .await
                .expect("evidence on orders")
                .expect("orders ran");
            let message = error.message.expect("reported message");
            assert_eq!(
                (
                    error.error_code.as_deref(),
                    error.path.as_deref(),
                    message.chars().count()
                ),
                (Some("T002"), Some("models/orders.sql"), 2000),
                "{}: code, a portable path, and the bound",
                test_case.description
            );
            assert!(
                message.starts_with(test_case.expected_message) && message.ends_with('…'),
                "{}: secrets are redacted before the cut: {message}",
                test_case.description
            );
            assert!(
                matches!(
                    read(member, run_id, "customers").await,
                    Err(ControlError::Denied { .. })
                ),
                "{}: evidence on another node does not reach this one",
                test_case.description
            );
            let administrator = (&roles.context, credentials(&roles.administrator));
            let skipped = read(administrator, run_id, "customers")
                .await
                .expect("administrator")
                .expect("customers was selected");
            assert_eq!(
                (skipped.message, skipped.path),
                (None, None),
                "{}: no reported text and no published path",
                test_case.description
            );
            for (run_id, node) in [(run_id, "products"), (Uuid::from_u128(9), "orders")] {
                assert!(
                    read(administrator, run_id, node)
                        .await
                        .expect("administrator")
                        .is_none(),
                    "{}: a node outside the run and an unknown run are not found",
                    test_case.description
                );
            }
            let stored = rows(
                store,
                "SELECT payload FROM recall_events WHERE invocation_id=$1 AND event_type='run_node_finished'",
                vec![Parameter::Text(format!("scheduler/{run_id}"))],
            )
            .await;
            assert!(
                stored.len() == 2
                    && stored
                        .iter()
                        .all(|row| !row.to_string().contains("orders-d")),
                "{}: the stored fact never held the secret",
                test_case.description
            );
            let audited = rows(
                store,
                "SELECT count(*) AS reads FROM sqb_security_audit WHERE action='error.read'",
                vec![],
            )
            .await;
            assert_eq!(
                audited[0]["reads"].as_i64(),
                Some(4),
                "{}: every allowed read is audited",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

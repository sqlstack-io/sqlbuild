//! A run's node and check events name their node in Recall's resource columns, so a run's event list
//! can say which node each one is about, including events stored before they carried those columns.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::execution::{change_run::change_run, claim_next_run::claim_next_run};
use crate::runs::models::{NodeOutcome, RunAuthority, RunChange, RunChangeRequest};
use crate::runs::tests::helpers::{queue, request, snapshot, snapshot_node};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::helpers::read_recall_events;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_history::_helpers::run_recall::node_of;
use crate::warehouse_history::main::read_person_run_events::read_person_run_events;

const NODE_EVENTS: [&str; 4] = [
    "run_node_started",
    "run_node_finished",
    "run_check_started",
    "run_check_finished",
];

#[test]
fn given_node_and_check_reports_when_stored_and_read_then_each_event_names_its_node() {
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let fixture = open_test_store(backend).await.expect("required store");
            let store = &fixture.store;
            crate::store::main::migrate_store::migrate_store(store)
                .await
                .expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.nodes.insert(
                "model:payments".into(),
                snapshot_node("model", json!({"name": "payments"})),
            );
            input.nodes.insert(
                "check:payments_positive".into(),
                snapshot_node("check", json!({"name": "payments_positive"})),
            );
            let published = publish_snapshot(store, &input).await.expect("snapshot");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            queue(
                store,
                &request((location, published.snapshot_id), 1, 1, &["model:payments"]),
            )
            .await;
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("available");
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
            let node_started = report(RunChange::NodeStarted {
                node_key: "model:payments".into(),
            });
            for request in [
                report(RunChange::Started),
                node_started.clone(),
                report(RunChange::CheckStarted {
                    node_key: "check:payments_positive".into(),
                }),
                report(RunChange::CheckFinished {
                    node_key: "check:payments_positive".into(),
                    outcome: NodeOutcome::Success,
                }),
                report(RunChange::NodeFinished {
                    node_key: "model:payments".into(),
                    outcome: NodeOutcome::Success,
                    rows_affected: Some(3),
                    error_code: None,
                    skipped_because_of: None,
                    error_message: None,
                }),
            ] {
                assert!(change_run(store, worker, &request).await.expect("report"));
            }

            let stored: Vec<(String, Option<String>, Option<String>)> =
                read_recall_events(store, 0, 1000)
                    .await
                    .expect("events")
                    .iter()
                    .filter(|row| NODE_EVENTS.contains(&row["event_type"].as_str().unwrap_or("")))
                    .map(|row| {
                        let text = |field: &str| row[field].as_str().map(str::to_owned);
                        (
                            text("event_type").unwrap_or_default(),
                            text("resource_kind"),
                            text("resource_name"),
                        )
                    })
                    .collect();
            let named = |event: &str, kind: &str, name: &str| {
                (
                    event.to_owned(),
                    Some(kind.to_owned()),
                    Some(name.to_owned()),
                )
            };
            assert_eq!(
                stored,
                [
                    named("run_node_started", "model", "payments"),
                    named("run_check_started", "check", "payments_positive"),
                    named("run_check_finished", "check", "payments_positive"),
                    named("run_node_finished", "model", "payments"),
                ],
                "{backend:?}: each node and check event stores its node in the resource columns"
            );
            assert!(
                !change_run(store, worker, &node_started)
                    .await
                    .expect("retried report"),
                "{backend:?}: a retried report is the same report"
            );

            // An event stored before node events carried resource columns: the old payload, no columns.
            let transaction = begin(store, TransactionMode::Write).await.expect("begin");
            let (transaction, _) = query(
                transaction,
                "UPDATE recall_events SET resource_kind=NULL, resource_name=NULL, payload=$1 WHERE event_id=$2",
                vec![
                    Parameter::Json(json!({"change": node_started})),
                    Parameter::Text(node_started.event_id.to_string()),
                ],
            )
            .await
            .expect("store the old shape");
            commit(transaction).await.expect("commit");
            assert!(
                !change_run(store, worker, &node_started)
                    .await
                    .expect("retried report against the old shape"),
                "{backend:?}: a report stored by an older server still matches its retry"
            );

            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let events = read_person_run_events(
                store,
                (&context, admin),
                ((location, "development", claim.request.run_id), (0, 1000)),
                DefaultRole::None,
            )
            .await
            .expect("run events")
            .expect("run");
            let read: Vec<(String, Option<String>, Option<String>)> = events
                .iter()
                .filter(|event| NODE_EVENTS.contains(&event.event_type.as_str()))
                .map(|event| {
                    (
                        event.event_type.clone(),
                        event.resource_kind.clone(),
                        event.resource_name.clone(),
                    )
                })
                .collect();
            assert_eq!(
                read, stored,
                "{backend:?}: the old event is named from its payload"
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_event_payloads_when_reading_their_node_then_only_node_identities_are_taken() {
    struct Case {
        description: &'static str,
        payload: serde_json::Value,
        expected: Option<(&'static str, &'static str)>,
    }
    let cases = [
        Case {
            description: "a scheduler change keyed <kind>:<name>",
            payload: json!({"change": {"change": {"kind": "node_started", "node_key": "seed:region_codes"}}}),
            expected: Some(("seed", "region_codes")),
        },
        Case {
            description: "a payload stored as JSON text",
            payload: json!(json!({"change": {"change": {"node_key": "model:orders"}}}).to_string()),
            expected: Some(("model", "orders")),
        },
        Case {
            description: "a node key without a kind names no node",
            payload: json!({"change": {"change": {"node_key": "orders"}}}),
            expected: None,
        },
        Case {
            description: "an audit names the node it is attached to",
            payload: json!({"attachment_kind": "seed", "target_name": "region_codes",
                "attached_target_kind": "seed", "attached_target_name": "region_codes", "attached_column_name": "code"}),
            expected: Some(("seed", "region_codes")),
        },
        Case {
            description: "an audit without an attached target names its own target",
            payload: json!({"attachment_kind": "model", "target_name": "orders"}),
            expected: Some(("model", "orders")),
        },
        Case {
            description: "a run-scope audit has no node",
            payload: json!({"attachment_kind": "end"}),
            expected: None,
        },
        Case {
            description: "no payload",
            payload: serde_json::Value::Null,
            expected: None,
        },
    ];
    for case in cases {
        assert_eq!(
            node_of(&case.payload),
            case.expected
                .map(|(kind, name)| (kind.to_owned(), name.to_owned())),
            "{}",
            case.description
        );
    }
}

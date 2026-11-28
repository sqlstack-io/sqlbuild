//! A republish needs its permission, names who asked, reports its publish and reads its sets back.

use crate::api::tests::helpers::check_in;
use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::api::tests::plans_helpers::{person, register_instance};
use crate::api::tests::test_types::{RepublishTestCase, republish_failed, republish_published};
use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, BuiltinRole, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::person_reads::read_person_discovered::read_person_discovered;
use crate::platform::main::work::{
    claim_location_work::claim_location_work, complete_location_work::complete_location_work,
    read_republish::read_republish, request_location_work::request_location_work,
    request_republish::request_republish,
};
use crate::platform::models::{
    DiscoveredNodeReadback, LocationWork, LocationWorkOutcome, LocationWorkRequest,
    LocationWorkResult, LocationWorkScope, PersonWorkRequest, RepublishState,
};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_republish_when_requested_claimed_and_completed_then_it_is_authorized_audited_and_read_back()
 {
    let test_cases = [
        RepublishTestCase {
            description: "SQLite republish that published",
            backend: Backend::Sqlite,
            publishes: true,
            outcome: republish_published,
            expected_state: RepublishState::Published,
            expected_error: None,
        },
        RepublishTestCase {
            description: "SQLite republish that failed",
            backend: Backend::Sqlite,
            publishes: false,
            outcome: republish_failed,
            expected_state: RepublishState::Failed,
            expected_error: Some("orders does not parse"),
        },
        RepublishTestCase {
            description: "PostgreSQL republish that published",
            backend: Backend::Postgres,
            publishes: true,
            outcome: republish_published,
            expected_state: RepublishState::Published,
            expected_error: None,
        },
        RepublishTestCase {
            description: "PostgreSQL republish that failed",
            backend: Backend::Postgres,
            publishes: false,
            outcome: republish_failed,
            expected_state: RepublishState::Failed,
            expected_error: Some("orders does not parse"),
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("store");
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
                .expect("admin");
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            let mut input = snapshot(location);
            publish_snapshot(store, &input).await.expect("publish");
            check_in(store, location, "development").await;
            let viewer = person(
                store,
                (&context, admin),
                ("orders-viewer", location),
                &[BuiltinRole::Viewer],
            )
            .await;
            let operator = person(
                store,
                (&context, admin),
                ("orders-deploy", location),
                &[BuiltinRole::Operator],
            )
            .await;
            let asked = PersonWorkRequest {
                request_id: Uuid::now_v7(),
                reason: "deploy 0.58.0".into(),
            };
            let ids = (location, "development", asked.request_id);

            assert!(
                matches!(
                    request_republish(
                        store,
                        (&context, Some(viewer)),
                        (location, "development"),
                        &asked
                    )
                    .await,
                    Err(ControlError::Denied { .. })
                ),
                "{}: a viewer may not republish",
                test_case.description
            );
            let connection = LocationWorkRequest {
                request_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                reason: "Check orders connectivity".into(),
                work: LocationWork::CheckConnection {},
            };
            request_location_work(store, (&context, None), &connection)
                .await
                .expect("older work");
            assert!(
                request_republish(
                    store,
                    (&context, Some(operator)),
                    (location, "development"),
                    &asked
                )
                .await
                .expect("request"),
                "{}: an operator may republish",
                test_case.description
            );
            assert!(
                !request_republish(
                    store,
                    (&context, Some(operator)),
                    (location, "development"),
                    &asked
                )
                .await
                .expect("retry"),
                "{}: a retried request queues nothing",
                test_case.description
            );
            assert!(
                matches!(
                    read_republish(store, (&context, Some(viewer)), ids, 0).await,
                    Err(ControlError::Denied { .. })
                ),
                "{}: a viewer may not read a republish",
                test_case.description
            );
            let mut poll = register_instance(store, location).await;
            poll.scope = LocationWorkScope::RepublishOnly;
            let claim = claim_location_work(store, location, &poll)
                .await
                .expect("poll")
                .expect("claim");
            assert_eq!(
                claim.request.work,
                LocationWork::Republish {
                    requested_by: "orders-deploy".into()
                },
                "{}: a failing location takes the republish ahead of older work, naming the caller",
                test_case.description
            );
            let foreign = LocationWorkResult {
                claim_id: claim.claim_id,
                session_id: poll.session_id,
                outcome: LocationWorkOutcome::Published {
                    snapshot_id: Uuid::now_v7(),
                    generation: 2,
                },
            };
            assert!(
                complete_location_work(store, (location, asked.request_id), &foreign)
                    .await
                    .is_err(),
                "{}: only this target's snapshot counts as published",
                test_case.description
            );
            input.generation = 2;
            input.documents.insert(
                "discovered".into(),
                json!({"orders_plan": {"status": "ok", "digest": "d-2",
                    "result": {"items": [{"id": "load", "depends_on": [], "args": {"region": "eu", "attempt": 2}}],
                        "tags": {"release": "0.58.0", "max_parallel": 4}}}}),
            );
            input.documents.insert(
                "discovered_assets".into(),
                json!({"orders_plan": ["load_orders"]}),
            );
            let published = publish_snapshot(store, &input).await.expect("republish");
            let outcome = (test_case.outcome)(published.snapshot_id);
            complete_location_work(
                store,
                (location, asked.request_id),
                &LocationWorkResult { outcome, ..foreign },
            )
            .await
            .expect("complete");
            let status = read_republish(store, (&context, Some(operator)), ids, 5)
                .await
                .expect("read")
                .expect("status");
            assert_eq!(
                (
                    status.state,
                    status.error.as_deref(),
                    status.snapshot_id.is_some()
                ),
                (
                    test_case.expected_state,
                    test_case.expected_error,
                    test_case.publishes
                ),
                "{}",
                test_case.description
            );
            let readback = read_person_discovered(
                store,
                (&context, viewer),
                (location, "development"),
                DefaultRole::None,
            )
            .await
            .expect("a viewer reads the sets back")
            .expect("published");
            let expected_tags: BTreeMap<String, String> = [
                ("max_parallel".to_owned(), "4".to_owned()),
                ("release".to_owned(), "0.58.0".to_owned()),
            ]
            .into();
            let expected_nodes = vec![DiscoveredNodeReadback {
                key: "asset:load_orders[load]".into(),
                asset: "load_orders".into(),
                step: "load".into(),
                args: [
                    ("attempt".to_owned(), "2".to_owned()),
                    ("region".to_owned(), "eu".to_owned()),
                ]
                .into(),
            }];
            assert_eq!(
                (
                    readback.generation,
                    readback.sets[0].items,
                    &readback.sets[0].tags,
                    &readback.sets[0].assets,
                    &readback.sets[0].nodes
                ),
                (
                    2,
                    1,
                    &expected_tags,
                    &vec!["load_orders".to_owned()],
                    &expected_nodes
                ),
                "{}: the readback shows the new tags, and the node each item became, with its args",
                test_case.description
            );
            let (transaction, audit) = query(
                begin(store, TransactionMode::Read).await.expect("read audit"),
                "SELECT actor, after_json FROM sqb_security_audit WHERE action='location.work_requested' ORDER BY storage_id DESC LIMIT 1",
                vec![],
            )
            .await
            .expect("audit");
            commit(transaction).await.expect("commit");
            assert_eq!(
                (
                    &audit[0]["actor"],
                    &audit[0]["after_json"]["work"]["requested_by"],
                    &audit[0]["after_json"]["reason"]
                ),
                (
                    &json!(format!("principal:{operator}")),
                    &json!("orders-deploy"),
                    &json!("deploy 0.58.0")
                ),
                "{}: the event names the caller and the reason",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

//! A group orders a warehouse round trip by its nearest lineage, SQL to asset to loader to models, on both stores.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    execution::advance_groups::advance_groups, execution::claim_next_run::claim_next_run,
    requests::request_job_run::request_job_run,
};
use crate::runs::models::{JobRunRequest, NodeOutcome, RunOutcome};
use crate::runs::tests::claim_helpers::finish;
use crate::runs::tests::helpers::{snapshot, snapshot_node};
use crate::runs::tests::test_types::RoundTripGroupTestCase;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_round_trip_split_across_jobs_when_the_group_runs_then_members_follow_the_loader_edge() {
    let test_cases = [
        RoundTripGroupTestCase {
            description: "SQLite: the asset waits for SQL, the loader for the asset, the model for the loader",
            backend: Backend::Sqlite,
            expected_waits: json!({
                "orders_job": [],
                "score_job": ["orders_job"],
                "ingest_job": ["score_job"],
                "report_job": ["ingest_job", "orders_job"],
            }),
            expected_order: &["orders_job", "score_job", "ingest_job", "report_job"],
        },
        RoundTripGroupTestCase {
            description: "PostgreSQL: the asset waits for SQL, the loader for the asset, the model for the loader",
            backend: Backend::Postgres,
            expected_waits: json!({
                "orders_job": [],
                "score_job": ["orders_job"],
                "ingest_job": ["score_job"],
                "report_job": ["ingest_job", "orders_job"],
            }),
            expected_order: &["orders_job", "score_job", "ingest_job", "report_job"],
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.nodes = BTreeMap::from([
                (
                    "model:orders".into(),
                    snapshot_node("model", json!({"deps": []})),
                ),
                (
                    "asset:score_orders".into(),
                    snapshot_node("asset", json!({"deps": ["model:orders"]})),
                ),
                (
                    "loader:partner_scores".into(),
                    snapshot_node("loader", json!({"deps": ["asset:score_orders"]})),
                ),
                (
                    "source:partner_scores".into(),
                    snapshot_node(
                        "source",
                        json!({"name": "partner_scores", "entry": {"loader": "partner_scores"}}),
                    ),
                ),
                (
                    "model:scored_orders".into(),
                    snapshot_node(
                        "model",
                        json!({"deps": ["model:orders", "source:partner_scores"]}),
                    ),
                ),
            ]);
            input.documents.insert(
                "jobs".into(),
                json!({
                    "orders_job": {"description": "Build orders", "selection": ["model:orders"]},
                    "score_job": {"description": "Score orders", "selection": ["asset:score_orders"]},
                    "ingest_job": {
                        "description": "Load partner scores",
                        "selection": ["loader:partner_scores", "source:partner_scores"]
                    },
                    "report_job": {
                        "description": "Build scored orders",
                        "selection": ["model:scored_orders"]
                    }
                }),
            );
            input.documents.insert(
                "groups".into(),
                json!({"scoring_job_group": {
                    "description": "Score orders through the partner and report on them",
                    "selection": [
                        "loader:partner_scores", "model:orders", "model:scored_orders",
                        "source:partner_scores", "asset:score_orders"
                    ],
                    "jobs": test_case.expected_waits,
                }}),
            );
            publish_snapshot(store, &input)
                .await
                .unwrap_or_else(|error| panic!("{}: publish: {error:?}", test_case.description));
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: Uuid::now_v7(),
                    location_id: location,
                    target: "development".into(),
                    job_key: "scoring_job_group".into(),
                    reason: "Score the orders".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("request group");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            let mut order: Vec<String> = Vec::new();
            for _ in test_case.expected_order {
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("advance group");
                let claim = claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim member")
                    .unwrap_or_else(|| panic!("{}: a member is ready", test_case.description));
                assert!(
                    claim_next_run(store, &lease, &limits, Some(scope))
                        .await
                        .expect("claim another")
                        .is_none(),
                    "{}: one member at a time along the chain",
                    test_case.description
                );
                order.push(claim.request.job_key.clone().expect("member"));
                finish(store, &claim, (NodeOutcome::Success, RunOutcome::Success)).await;
            }
            assert_eq!(order, test_case.expected_order, "{}", test_case.description);
            close_test_store(fixture).await;
        });
    }
}

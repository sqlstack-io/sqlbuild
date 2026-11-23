//! A selection's estimated query cost through the public API: job by node, the period before, folders and daily series.

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::api::tests::helpers::stop_api;
use crate::api::tests::test_types::{CostFolderTestCase, CostSelectionTestCase};
use crate::api::tests::warehouse_history_helpers::{
    batch, cost_facts as facts, cost_observations, cost_selection_observations as observations,
    get, login, percent_encoded as urlencoding, seed_target, series_totals as totals,
    start_password_api,
};
use crate::runs::tests::helpers::{request, request_run, rows, snapshot};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_history::_helpers::cost_selection::cost_folder;
use crate::warehouse_metadata::main::ingest::ingest_resource_costs::ingest_resource_costs;

#[test]
fn given_job_and_manual_builds_when_people_read_a_selections_cost_then_it_splits_job_by_node_with_daily_series()
 {
    let test_cases = [
        CostSelectionTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_all: vec![
                (
                    json!("daily"),
                    json!("model:orders"),
                    json!("staging"),
                    0.10,
                    0.20,
                ),
                (
                    json!("daily"),
                    json!("model:customers"),
                    json!(""),
                    0.05,
                    0.0,
                ),
                (
                    Value::Null,
                    json!("model:orders"),
                    json!("staging"),
                    0.04,
                    0.0,
                ),
            ],
            expected_series: vec![0.15, 0.04, 0.14, 0.05],
        },
        CostSelectionTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_all: vec![
                (
                    json!("daily"),
                    json!("model:orders"),
                    json!("staging"),
                    0.10,
                    0.20,
                ),
                (
                    json!("daily"),
                    json!("model:customers"),
                    json!(""),
                    0.05,
                    0.0,
                ),
                (
                    Value::Null,
                    json!("model:orders"),
                    json!("staging"),
                    0.04,
                    0.0,
                ),
            ],
            expected_series: vec![0.15, 0.04, 0.14, 0.05],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture_store) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            let mut input = snapshot(location);
            input.generation = 2;
            input.nodes = [
                ("orders", "sql/models/staging/orders.sql"),
                ("customers", "sql/models/customers.sql"),
            ]
            .into_iter()
            .map(|(name, path)| {
                (
                    format!("model:{name}"),
                    SnapshotNode {
                        kind: "model".into(),
                        definition: json!({"materialization": "table", "path": path}),
                        raw_sql: None,
                        compiled_sql: Some("select 1 as id".into()),
                        python_source: None,
                    },
                )
            })
            .collect();
            let published = publish_snapshot(store, &input).await.expect("snapshot");
            let mut runs = Vec::new();
            for id in [0x1654_u128, 0x1655] {
                let mut daily =
                    request((location, published.snapshot_id), id, 0, &["model:orders"]);
                daily.job_key = Some("daily".into());
                assert!(request_run(store, &daily).await.expect("daily run"));
                runs.push(daily.run_id);
            }
            let mut costs = batch(
                session_id,
                "resource_costs",
                cost_observations(&observations(runs[0], runs[1])),
            );
            // The batch reads the second publication's state.
            costs.generation = 2;
            ingest_resource_costs(store, (location, "development"), &costs)
                .await
                .expect("cost ingestion");
            assert!(
                !rows(
                    store,
                    "SELECT event_id FROM sqb_warehouse_resource_costs",
                    vec![]
                )
                .await
                .is_empty()
            );

            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let path = format!("/api/control/locations/{location}/targets/development/cost");

            let (status, all) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/selection?days=7"),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{}: {all}", test_case.description);
            assert_eq!(
                facts(&all),
                test_case.expected_all,
                "{}: job by node, the folder under sql/models, and the period before",
                test_case.description
            );
            assert_eq!(
                (
                    all["dates"].as_array().map(Vec::len),
                    all["runs"].clone(),
                    all["price"]["source"].clone()
                ),
                (Some(7), json!(2), json!("configured")),
                "{}",
                test_case.description
            );
            assert_eq!(
                (totals(&all["job_daily"]), totals(&all["folder_daily"])),
                (vec![0.04, 0.15], vec![0.05, 0.14]),
                "{}: daily series by job (none, then daily) and top-level folder (top, then staging)",
                test_case.description
            );

            let (_, orders) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/selection?select=model:orders"),
            )
            .await;
            assert_eq!(
                facts(&orders).len(),
                2,
                "{}: only the selected node",
                test_case.description
            );
            let (status, refused) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/selection?select=model:invoices"),
            )
            .await;
            assert_eq!(
                (
                    status,
                    refused["error"]["message"].is_string(),
                    refused["facts"].clone()
                ),
                (StatusCode::OK, true, json!([])),
                "{}: a refused selection says why",
                test_case.description
            );

            let groups = json!([{"job": "daily"}, {"manual": true}, {"folder": "staging"}, {"node_key": "model:customers"}]);
            let (_, batch_read) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/series?groups={}", urlencoding(&groups.to_string())),
            )
            .await;
            assert_eq!(
                totals(&batch_read["series"]),
                test_case.expected_series,
                "{}: each group's daily credits",
                test_case.description
            );
            let too_many = Value::Array(vec![json!({}); 201]).to_string();
            let (status, _) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/series?groups={}", urlencoding(&too_many)),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{}: at most 200 groups",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}

#[test]
fn given_published_paths_when_costs_are_grouped_then_each_node_is_in_its_folder() {
    let test_cases = [
        CostFolderTestCase {
            description: "a model under sql/models",
            path: Some("sql/models/staging/partners/orders.sql"),
            expected_folder: "staging/partners",
        },
        CostFolderTestCase {
            description: "a model at the top of sql/models",
            path: Some("sql/models/orders.sql"),
            expected_folder: "",
        },
        CostFolderTestCase {
            description: "a Python node keeps its root",
            path: Some("python/nodes/exports/asset.py"),
            expected_folder: "python/nodes/exports",
        },
        CostFolderTestCase {
            description: "a node no longer published",
            path: None,
            expected_folder: "other",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            cost_folder(test_case.path),
            test_case.expected_folder,
            "{}",
            test_case.description
        );
    }
}

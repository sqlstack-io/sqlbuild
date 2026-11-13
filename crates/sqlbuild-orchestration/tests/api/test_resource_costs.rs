//! Estimated query cost through the public API, from the latest observation of each run.

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::tests::helpers::stop_api;
use crate::api::tests::test_types::WarehouseMetadataTestCase;
use crate::api::tests::warehouse_history_helpers::{
    batch, cost_groups, cost_observations, fixture, get, login, seed_target, start_password_api,
};
use crate::runs::tests::helpers::{request, request_run, rows};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest::ingest_resource_costs::ingest_resource_costs;

#[test]
fn given_refreshed_and_unpriced_costs_when_people_read_cost_then_latest_observations_are_grouped() {
    let test_cases = [
        WarehouseMetadataTestCase {
            description: "SQLite cost reads",
            backend: Backend::Sqlite,
            expected_observations: 11,
        },
        WarehouseMetadataTestCase {
            description: "PostgreSQL cost reads",
            backend: Backend::Postgres,
            expected_observations: 11,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture_store = open_test_store(test_case.backend).await.expect("store");
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            let snapshot_id: Uuid = serde_json::from_value(
                rows(store, "SELECT snapshot_id FROM sqb_snapshots", vec![]).await[0]
                    ["snapshot_id"]
                    .clone(),
            )
            .expect("snapshot");
            let mut daily = request((location, snapshot_id), 0x1302, 0, &["orders", "customers"]);
            daily.job_key = Some("daily".into());
            assert!(request_run(store, &daily).await.expect("daily run"));

            let records = cost_observations(&fixture("resource_cost_history.json"));
            assert_eq!(records.len(), test_case.expected_observations);
            ingest_resource_costs(
                store,
                (location, "development"),
                &batch(session_id, "resource_costs", records),
            )
            .await
            .expect("real cost ingestion");

            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let path = format!("/api/control/locations/{location}/targets/development");

            let (status, week) = get(&client, (&api.url, &viewer), &format!("{path}/cost")).await;
            assert_eq!(status, StatusCode::OK, "{}: {week}", test_case.description);
            assert_eq!(
                (
                    week["runs"].clone(),
                    week["incomplete_runs"].clone(),
                    week["usd"].clone()
                ),
                (json!(2), json!(0), Value::Null),
                "the refresh replaced the partial figures, and an unpriced run drops the dollar total"
            );
            assert!(
                (week["credits"].as_f64().expect("credits") - 0.6).abs() < 1e-9,
                "only each run's latest attempt counts, and the run's own statements count once: {week}"
            );
            assert_eq!(
                cost_groups(&week, "by_job"),
                [
                    (json!("daily"), 0.4, json!(1.2), 1),
                    (Value::Null, 0.2, Value::Null, 1)
                ],
                "the server run's job, and none for a build from the command line"
            );
            assert_eq!(
                cost_groups(&week, "by_warehouse_size"),
                [
                    (json!("Small"), 0.4, json!(1.2), 1),
                    (json!("X-Small"), 0.2, Value::Null, 1)
                ]
            );
            assert_eq!(
                cost_groups(&week, "by_node"),
                [
                    (json!("model:orders"), 0.45, Value::Null, 2),
                    (json!("model:customers"), 0.1, json!(0.3), 1),
                    (json!("run:development"), 0.05, json!(0.15), 1)
                ],
                "keyed like catalog node keys"
            );

            let (_, month) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/cost?days=30"),
            )
            .await;
            assert_eq!(
                month["runs"],
                json!(3),
                "a longer window reaches older runs"
            );

            let (_, orders) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/nodes/model:orders/cost?days=30"),
            )
            .await;
            let builds: Vec<(Value, Value, Value)> = orders
                .as_array()
                .expect("builds")
                .iter()
                .map(|build| {
                    (
                        build["run_id"].clone(),
                        build["status"].clone(),
                        build["orchestration_run_id"].clone(),
                    )
                })
                .collect();
            assert_eq!(
                builds,
                [
                    (
                        json!("build-1"),
                        json!("complete"),
                        json!(daily.run_id.to_string())
                    ),
                    (json!("build-2"), json!("complete"), Value::Null),
                    (json!("build-3"), json!("complete"), Value::Null)
                ],
                "newest first, each build once with its latest status"
            );
            assert!((orders[0]["credits"].as_f64().expect("credits") - 0.25).abs() < 1e-9);

            let (_, run_cost) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}/runs/{}/cost", daily.run_id),
            )
            .await;
            let mut ids: Vec<&str> = run_cost
                .as_array()
                .expect("rows")
                .iter()
                .map(|row| row["observation_id"].as_str().expect("id"))
                .collect();
            ids.sort_unstable();
            assert_eq!(
                ids,
                [
                    "build-1-customers-2",
                    "build-1-orders-2",
                    "build-1-run-2",
                    "build-1-setup-2"
                ],
                "only the latest attempt of each resource"
            );
            assert_eq!(
                get(
                    &client,
                    (&api.url, &viewer),
                    &format!("{path}/runs/{}/cost", Uuid::now_v7())
                )
                .await
                .0,
                StatusCode::NOT_FOUND
            );
            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}

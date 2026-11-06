//! Microbatch coverage through the public API: completed segments, holes and stale versions.

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::api::tests::helpers::stop_api;
use crate::api::tests::test_types::WarehouseMetadataTestCase;
use crate::api::tests::warehouse_history_helpers::{
    batch, coverage_batch, fixture, get, login, records, seed_target, start_password_api,
};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest_microbatches::ingest_microbatches;

const HOUR_0: &str = "2026-01-01 00:00:00+00";
const HOUR_1: &str = "2026-01-01T09:00:00+08:00";
const HOUR_2: &str = "2026-01-01T02:00:00+00:00";
const HOUR_3: &str = "2026-01-01T03:00:00+00:00";
const HOUR_4: &str = "2026-01-01T04:00:00+00:00";
const HOUR_5: &str = "2026-01-01T05:00:00+00:00";
const HOUR_6: &str = "2026-01-01T06:00:00+00:00";
const HOUR_7: &str = "2026-01-01T07:00:00+00:00";

#[test]
fn given_microbatch_history_when_people_read_coverage_then_holes_and_stale_versions_are_marked() {
    let test_cases = [
        WarehouseMetadataTestCase {
            description: "SQLite microbatch coverage",
            backend: Backend::Sqlite,
            expected_observations: 20,
        },
        WarehouseMetadataTestCase {
            description: "PostgreSQL microbatch coverage",
            backend: Backend::Postgres,
            expected_observations: 20,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture_store = open_test_store(test_case.backend).await.expect("store");
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            let history = records(
                "microbatch_history.json",
                &fixture("microbatch_coverage.json")["microbatches"],
            );
            assert_eq!(history.len(), test_case.expected_observations);
            ingest_microbatches(
                store,
                (location, "development"),
                &batch(session_id, "microbatches", history),
            )
            .await
            .expect("real microbatch ingestion");

            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let path =
                format!("/api/control/locations/{location}/targets/development/microbatches");
            let read = |model: &str| format!("{path}/{model}");

            let (status, orders) = get(&client, (&api.url, &viewer), &read("orders")).await;
            assert_eq!(status, StatusCode::OK, "{}", test_case.description);
            let early_gap = coverage_batch(HOUR_2, HOUR_3, "gap", Value::Null);
            let late_gap = coverage_batch(HOUR_5, HOUR_6, "gap", Value::Null);
            assert_eq!(
                orders,
                json!({
                    "model": "orders",
                    "target_name": "orders_\u{1f9fe}",
                    "physical_generation_id": "orders-generation-1",
                    "cursor_column": "ordered_at",
                    "cursor_type": "timestamp",
                    "batch_size": "1 hour",
                    "earliest": HOUR_0,
                    "contiguous_frontier": HOUR_2,
                    "known_gaps": [early_gap, late_gap],
                    "batches": [
                        coverage_batch(HOUR_0, HOUR_1, "replay_required", json!("run-1")),
                        coverage_batch(HOUR_1, HOUR_2, "replay_required", json!("run-2")),
                        early_gap,
                        coverage_batch(HOUR_3, HOUR_4, "complete", json!("run-5")),
                        coverage_batch(HOUR_4, HOUR_5, "complete", Value::Null),
                        late_gap,
                        coverage_batch(HOUR_6, HOUR_7, "complete", Value::Null),
                    ],
                    "batches_truncated": false,
                }),
                "bounds sort by instant; the newest completion and requirement win; other generations, scopes and models never appear"
            );

            let (_, newest) = get(
                &client,
                (&api.url, &viewer),
                &format!("{}?limit=2", read("orders")),
            )
            .await;
            assert_eq!(
                (&newest["batches"], &newest["batches_truncated"]),
                (
                    &json!([
                        late_gap,
                        coverage_batch(HOUR_6, HOUR_7, "complete", Value::Null)
                    ]),
                    &json!(true)
                )
            );
            assert_eq!(
                newest["known_gaps"],
                json!([early_gap, late_gap]),
                "gaps are never truncated"
            );

            let (_, events) = get(&client, (&api.url, &viewer), &read("events")).await;
            assert_eq!(
                (
                    &events["earliest"],
                    &events["contiguous_frontier"],
                    &events["batches"]
                ),
                (
                    &json!("2"),
                    &json!("11"),
                    &json!([
                        coverage_batch("2", "9", "complete", json!("run-a")),
                        coverage_batch("9", "10", "complete", json!("run-b")),
                        coverage_batch("10", "11", "complete", json!("run-c")),
                    ])
                ),
                "integer bounds sort by value, not text"
            );

            let (_, payments) = get(&client, (&api.url, &viewer), &read("payments")).await;
            assert_eq!(
                payments["batches"],
                json!([
                    coverage_batch(
                        "2026-01-01T00:00:00+00:00",
                        "2026-01-01T01:00:00+00:00",
                        "complete",
                        json!("run-p3")
                    ),
                    coverage_batch(
                        "2026-01-01T01:00:00+00:00",
                        "2026-01-01T02:00:00+00:00",
                        "complete",
                        json!("run-p2")
                    ),
                ]),
                "a completion at a third version ends the requirement, as the executor decides"
            );

            let (status, untracked) = get(&client, (&api.url, &viewer), &read("inventory")).await;
            assert_eq!((status, untracked), (StatusCode::OK, Value::Null));
            assert_eq!(
                get(
                    &client,
                    (&api.url, &viewer),
                    &read("orders").replace("development", "preview")
                )
                .await
                .0,
                StatusCode::FORBIDDEN
            );
            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}

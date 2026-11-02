//! A reset during optional cleanup cannot tear down the HTTP server.

use std::time::Duration;

use serde_json::Value;

use crate::api::tests::changes_helpers::reset_proxy;
use crate::api::tests::helpers::{start_api, stop_api};
use crate::api::tests::test_types::WorkerRedirectTestCase;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_statistics_cleanup_when_connection_resets_then_http_survives() {
    let test_cases = [WorkerRedirectTestCase {
        description: "PostgreSQL cleanup TCP reset",
        expected_status: reqwest::StatusCode::OK,
    }];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(Backend::Postgres)
                .await
                .expect("isolated store");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let ControlStore::Postgres(pool) = store else {
                panic!("PostgreSQL case")
            };
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/warehouse_statistics.json",
            );
            let data: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
                    .expect("statistics fixture");
            let fault = &data["cleanup_interruption"];
            sqlx_core::raw_sql::raw_sql(fault["block_sql"].as_str().expect("block SQL"))
                .execute(pool)
                .await
                .expect("pause actual cleanup");
            let proxy = reset_proxy(store).await;
            let api = start_api(&proxy.store).await;
            tokio::time::timeout(
                Duration::from_secs(6),
                sqlx_core::raw_sql::raw_sql(fault["wait_sql"].as_str().expect("wait SQL"))
                    .execute(pool),
            )
            .await
            .expect("bounded active cleanup wait")
            .expect("actual prune is active");
            proxy.reset.send_modify(|epoch| *epoch += 1);
            tokio::time::sleep(Duration::from_millis(100)).await;
            sqlx_core::raw_sql::raw_sql(fault["restore_sql"].as_str().expect("restore SQL"))
                .execute(pool)
                .await
                .expect("remove fault");
            let response = reqwest::Client::new()
                .get(format!("{}/api/openapi.json", api.url))
                .send()
                .await
                .expect("HTTP survives cleanup reset");
            assert_eq!(
                response.status(),
                test_case.expected_status,
                "{}",
                test_case.description
            );
            let schema: Value = response.json().await.expect("OpenAPI after cleanup reset");
            assert!(
                schema["paths"]
                    .get("/api/control/locations/{location_id}/targets/{target}/statistics")
                    .is_some()
            );
            stop_api(api).await;
            close_store(proxy.store).await;
            proxy.shutdown.send(()).expect("stop proxy");
            proxy.task.await.expect("proxy stopped");
            close_test_store(fixture).await;
        });
    }
}

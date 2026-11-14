//! `sqb location health` reads a target's state with the location credential and fails a container health check when it is down or falling back.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use serde_json::{Value, json};
use sqlbuild_orchestration::snapshots::main::publish_remote_snapshot::publish_remote_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use uuid::Uuid;

use crate::admin_helpers::administer;
use crate::helpers::{api_ready, start_control};
use crate::test_types::LocationHealthTestCase;

#[test]
fn given_location_health_reports_when_checking_health_then_down_or_falling_back_locations_exit_nonzero()
 {
    let test_cases = [
        LocationHealthTestCase {
            description: "no instance has checked in",
            reports: &[],
            expected_state: "broken",
            expected_exit: 1,
        },
        LocationHealthTestCase {
            description: "a recent heartbeat",
            reports: &[r#"{"kind":"heartbeat"}"#],
            expected_state: "healthy",
            expected_exit: 0,
        },
        LocationHealthTestCase {
            description: "the newest load failed after a good connection check",
            reports: &[
                r#"{"kind":"connection","generation":2,"healthy":true}"#,
                r#"{"kind":"compile_failed","generation":3}"#,
            ],
            expected_state: "falling_back",
            expected_exit: 1,
        },
        LocationHealthTestCase {
            description: "a later load published and connected after an earlier failure",
            reports: &[
                r#"{"kind":"compile_failed","generation":3}"#,
                r#"{"kind":"connection","generation":4,"healthy":true}"#,
            ],
            expected_state: "healthy",
            expected_exit: 0,
        },
        LocationHealthTestCase {
            description: "the warehouse connection check failed",
            reports: &[r#"{"kind":"connection","generation":2,"healthy":false}"#],
            expected_state: "degraded",
            expected_exit: 0,
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    runtime.block_on(async {
        let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
        let url = api_ready(&mut control).await;
        let client = reqwest::Client::new();
        for test_case in test_cases {
            let location = administer(
                &control,
                &["create-location", &format!("orders-{}", Uuid::now_v7())],
            );
            let location_id = location["location_id"].as_str().expect("location UUID").to_owned();
            let expiry = (Utc::now() + Duration::hours(1)).to_rfc3339();
            let credential = administer(
                &control,
                &["issue-location-token", &location_id, "--expires-at", &expiry],
            );
            let bearer = credential["bearer"].as_str().expect("location bearer").to_owned();
            // Connection reports name a generation the location has published.
            for generation in 1..=4 {
                let snapshot = SnapshotInput {
                    location_id: location_id.parse().expect("location UUID"),
                    target: "development".into(),
                    generation,
                    git_revision: None,
                    git_dirty: false,
                    git_author: None,
                    image_id: None,
                    environment_hash: "orders-environment".into(),
                    sqb_version: env!("CARGO_PKG_VERSION").into(),
                    nodes: BTreeMap::from([(
                        "model:orders".into(),
                        SnapshotNode {
                            kind: "model".into(),
                            definition: json!({"materialization": "table"}),
                            raw_sql: Some("select 1 as order_id".into()),
                            compiled_sql: Some("select 1 as order_id".into()),
                            python_source: None,
                        },
                    )]),
                    documents: BTreeMap::new(),
                };
                publish_remote_snapshot(&url, &bearer, &snapshot)
                    .await
                    .expect("location publishes");
            }
            let session_id = Uuid::now_v7();
            for report in test_case.reports {
                let health: Value = serde_json::from_str(report).expect("health JSON");
                let response = client
                    .post(format!("{url}/api/locations/{location_id}/health"))
                    .bearer_auth(&bearer)
                    .json(&json!({"event_id": Uuid::now_v7(), "session_id": session_id, "target": "development", "health": health}))
                    .send()
                    .await
                    .expect("health report");
                let status = response.status();
                assert!(
                    status.is_success(),
                    "{}: {status} {}",
                    test_case.description,
                    response.text().await.unwrap_or_default()
                );
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            let output = std::process::Command::new(control.directory.path().join(format!("sqb{}", std::env::consts::EXE_SUFFIX)))
                .args(["location", "health", "--server", &url, "--location-id", &location_id, "--target", "development"])
                .current_dir(control.directory.path())
                .env("PATH", "")
                .env("SQLBUILD_LOCATION_TOKEN", &bearer)
                .output()
                .expect("location health process");
            let document: Value = serde_json::from_slice(&output.stdout).expect("JSON-only stdout");
            assert_eq!(
                (document["state"].as_str(), output.status.code()),
                (Some(test_case.expected_state), Some(test_case.expected_exit)),
                "{}: {}",
                test_case.description,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        #[cfg(unix)]
        crate::helpers::terminate(&mut control).await;
    });
}

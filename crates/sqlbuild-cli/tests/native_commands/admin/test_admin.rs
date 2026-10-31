//! Native bootstrap credentials authorize real publication and stop working after revocation.

#[path = "test_types.rs"]
mod test_types;

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::snapshots::main::publish_remote_snapshot::publish_remote_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use uuid::Uuid;

use crate::admin_helpers::{administer, audit_actions, invoke_admin, read_triggers};
use crate::helpers::{api_ready, start_control};
#[cfg(unix)]
use crate::test_admin::test_types::AdminDeliveryTestCase;
use crate::test_admin::test_types::{AdminTestCase, PrincipalAdminTestCase, TriggerAdminTestCase};

#[test]
fn given_a_standalone_control_server_when_bootstrapped_by_an_operator_then_scoped_publication_and_revocation_are_audited()
 {
    let test_cases = [AdminTestCase {
        description: "native operator bootstrap preserves labels and scopes credentials",
        location_name: "orders / café",
        expected_name: "orders / café",
        expected_actions: [
            "location.created",
            "location.created",
            "token.issued",
            "token.revoked",
            "trigger.changed",
            "trigger.changed",
            "trigger.changed",
            "trigger.changed",
            "trigger.changed",
        ],
    }];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
            let url = api_ready(&mut control).await;
            let first = administer(&control, &["create-location", test_case.location_name]);
            let second = administer(&control, &["create-location", "customers"]);
            let location_id =
                Uuid::parse_str(first["location_id"].as_str().expect("location UUID"))
                    .expect("UUID");
            let other_id = Uuid::parse_str(second["location_id"].as_str().expect("location UUID"))
                .expect("UUID");
            assert_ne!(location_id, other_id);
            assert_eq!(
                first["name"], test_case.expected_name,
                "{}",
                test_case.description
            );
            let expiry = (Utc::now() + Duration::hours(1)).to_rfc3339();
            let credential = administer(
                &control,
                &[
                    "issue-location-token",
                    &location_id.to_string(),
                    "--expires-at",
                    &expiry,
                ],
            );
            let bearer = credential["bearer"].as_str().expect("one-time bearer");
            assert_eq!(credential["identity"]["scope"]["kind"], "location");
            assert_eq!(
                credential["identity"]["scope"]["location_id"],
                location_id.to_string()
            );
            let mut snapshot = SnapshotInput {
                location_id,
                target: "development".into(),
                generation: 1,
                git_revision: None,
                git_dirty: false,
                git_author: None,
                image_id: None,
                environment_hash: "orders-environment".into(),
                sqb_version: env!("CARGO_PKG_VERSION").into(),
                nodes: BTreeMap::from([("model:orders".into(), SnapshotNode {
                    kind: "model".into(),
                    definition: serde_json::json!({"materialization": "table"}),
                    raw_sql: Some("select 1 as order_id".into()),
                    compiled_sql: Some("select 1 as order_id".into()),
                    python_source: None,
                })]),
                documents: BTreeMap::from([
                    ("jobs".into(), serde_json::json!({"orders": {"description": "Refresh orders.", "selection": ["model:orders"]}})),
                    ("triggers".into(), serde_json::json!({"daily_orders": {
                        "description": "Daily orders.", "job_key": "orders", "default_status": "paused",
                        "schedule": {"kind": "cron", "expression": "0 2 * * *", "timezone": "UTC"},
                    }})),
                ]),
            };
            let published = publish_remote_snapshot(&url, bearer, &snapshot)
                .await
                .expect("operator-issued credential publishes");
            assert_eq!(published.location_id, location_id);
            let trigger_id = read_triggers(&url, bearer, (location_id, "development")).await[0].trigger_id.to_string();
            for case in [
                TriggerAdminTestCase { description: "enable overrides the paused code default", change: "enable", cursor_arguments: &[], expected_status: "running", expected_cursor: None },
                TriggerAdminTestCase { description: "pause overrides an enabled trigger", change: "pause", cursor_arguments: &[], expected_status: "paused", expected_cursor: None },
                TriggerAdminTestCase { description: "reset-default restores the paused code default", change: "reset-default", cursor_arguments: &[], expected_status: "paused", expected_cursor: None },
                TriggerAdminTestCase { description: "set-cursor normalizes the timestamp offset", change: "set-cursor", cursor_arguments: &["--cursor", "2026-07-03T01:17:23+05:30"], expected_status: "paused", expected_cursor: Some("2026-07-02T19:47:23+00:00") },
                TriggerAdminTestCase { description: "reset-cursor removes the durable cursor", change: "reset-cursor", cursor_arguments: &[], expected_status: "paused", expected_cursor: None },
            ] {
                let event_id = Uuid::now_v7().to_string();
                let mut arguments = vec!["change-trigger", &trigger_id, case.change, "--event-id", &event_id, "--reason", "Order refresh maintenance"];
                arguments.extend_from_slice(case.cursor_arguments);
                for expected in [true, false] {
                    let result = administer(&control, &arguments);
                    assert_eq!(result["applied"], expected, "{}", case.description);
                    assert_eq!(result["event_id"], event_id);
                }
                let mut conflict = arguments.clone();
                conflict[6] = "Different request content";
                assert!(!invoke_admin(&control, &conflict).status.success(), "{}", case.description);
                let triggers = read_triggers(&url, bearer, (location_id, "development")).await;
                assert_eq!(triggers[0].status.as_str(), case.expected_status, "{}", case.description);
                assert_eq!(triggers[0].last_tick_for.map(|value| value.to_rfc3339()).as_deref(), case.expected_cursor, "{}", case.description);
            }
            snapshot.generation = 2;
            snapshot.documents.get_mut("triggers").expect("trigger document")["daily_orders"]["default_status"] = serde_json::json!("running");
            publish_remote_snapshot(&url, bearer, &snapshot).await.expect("publish a new default");
            assert_eq!(read_triggers(&url, bearer, (location_id, "development")).await[0].status.as_str(), "running", "reset-default removed the operator override");
            snapshot.location_id = other_id;
            assert!(matches!(
                publish_remote_snapshot(&url, bearer, &snapshot).await,
                Err(ClientError::Rejected(reqwest::StatusCode::FORBIDDEN))
            ));
            snapshot.location_id = location_id;
            snapshot.generation = 2;
            let token_id = credential["identity"]["token_id"]
                .as_str()
                .expect("credential ID");
            for expected in [true, false] {
                let revoked = administer(&control, &["revoke-token", token_id]);
                assert_eq!(revoked["revoked"], expected);
                assert_eq!(revoked["token_id"], token_id);
            }
            assert!(matches!(
                publish_remote_snapshot(&url, bearer, &snapshot).await,
                Err(ClientError::Rejected(reqwest::StatusCode::UNAUTHORIZED))
            ));
            let mut actions = audit_actions(&control, bearer);
            actions.sort();
            assert_eq!(
                actions, test_case.expected_actions,
                "{}",
                test_case.description
            );
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[test]
fn given_native_principal_administration_when_issuing_credentials_then_identity_is_audited_without_granting_location_permissions()
 {
    let test_cases = [
        PrincipalAdminTestCase {
            description: "a person credential has no implicit location authority",
            kind: "person",
            expected_name: "orders operator / café",
            expected_actions: ["principal.created", "token.issued", "token.revoked"],
        },
        PrincipalAdminTestCase {
            description: "a service account credential has no implicit location authority",
            kind: "service_account",
            expected_name: "orders automation",
            expected_actions: ["principal.created", "token.issued", "token.revoked"],
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
            let url = api_ready(&mut control).await;
            let principal = administer(
                &control,
                &[
                    "create-principal",
                    test_case.expected_name,
                    "--kind",
                    test_case.kind,
                ],
            );
            let principal_id = Uuid::parse_str(
                principal["principal_id"]
                    .as_str()
                    .expect(test_case.description),
            )
            .expect(test_case.description);
            assert_eq!(
                principal["name"], test_case.expected_name,
                "{}",
                test_case.description
            );
            assert_eq!(
                principal["kind"], test_case.kind,
                "{}",
                test_case.description
            );
            let expiry = (Utc::now() + Duration::hours(1)).to_rfc3339();
            let credential = administer(
                &control,
                &[
                    "issue-principal-token",
                    &principal_id.to_string(),
                    "--expires-at",
                    &expiry,
                ],
            );
            assert_eq!(
                credential["identity"]["scope"]["kind"], "principal",
                "{}",
                test_case.description
            );
            assert_eq!(
                credential["identity"]["scope"]["principal_id"],
                principal_id.to_string(),
                "{}",
                test_case.description
            );
            let bearer = credential["bearer"].as_str().expect(test_case.description);
            let snapshot = SnapshotInput {
                location_id: Uuid::now_v7(),
                target: "development".into(),
                generation: 1,
                git_revision: None,
                git_dirty: false,
                git_author: None,
                image_id: None,
                environment_hash: "orders-environment".into(),
                sqb_version: env!("CARGO_PKG_VERSION").into(),
                nodes: BTreeMap::new(),
                documents: BTreeMap::new(),
            };
            assert!(
                matches!(
                    publish_remote_snapshot(&url, bearer, &snapshot).await,
                    Err(ClientError::Rejected(reqwest::StatusCode::FORBIDDEN))
                ),
                "{}",
                test_case.description
            );
            let token_id = credential["identity"]["token_id"]
                .as_str()
                .expect(test_case.description);
            let revoked = administer(&control, &["revoke-token", token_id]);
            assert_eq!(revoked["revoked"], true, "{}", test_case.description);
            assert!(
                matches!(
                    publish_remote_snapshot(&url, bearer, &snapshot).await,
                    Err(ClientError::Rejected(reqwest::StatusCode::UNAUTHORIZED))
                ),
                "{}",
                test_case.description
            );
            assert_eq!(
                audit_actions(&control, bearer),
                test_case.expected_actions,
                "{}",
                test_case.description
            );
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[cfg(unix)]
#[test]
fn given_a_committed_admin_operation_when_json_output_is_lost_then_all_response_sizes_fail_without_claiming_rollback()
 {
    let test_cases = [
        AdminDeliveryTestCase {
            description: "small response",
            name_bytes: 8,
            expected_exit: 1,
            expected_rows: (1, 1),
        },
        AdminDeliveryTestCase {
            description: "Python compatibility drop window",
            name_bytes: 5000,
            expected_exit: 1,
            expected_rows: (1, 1),
        },
        AdminDeliveryTestCase {
            description: "large response",
            name_bytes: 9000,
            expected_exit: 1,
            expected_rows: (1, 1),
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("isolated administration directory");
        let binary = crate::helpers::isolated_binary(directory.path());
        let output = std::process::Command::new(binary)
            .args([
                "admin",
                "create-location",
                &"x".repeat(test_case.name_bytes),
            ])
            .current_dir(directory.path())
            .env("PATH", "")
            .env_remove("SQLBUILD_CONTROL_DATABASE_URL")
            .stdout(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .expect("failing output device"),
            )
            .output()
            .expect("native administration");
        assert_eq!(
            output.status.code(),
            Some(test_case.expected_exit),
            "{}",
            test_case.description
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "Opening control store...\nControl store ready\nControl administration JSON result delivery failed; changes may already be committed\n",
            "{}",
            test_case.description
        );
        let connection =
            rusqlite::Connection::open(directory.path().join(".sqlbuild/control.sqlite3"))
                .expect("inspect committed operation");
        let rows: (i64, i64) = connection.query_row("SELECT (SELECT COUNT(*) FROM sqb_locations), (SELECT COUNT(*) FROM sqb_security_audit)", [], |row| Ok((row.get(0)?, row.get(1)?))).expect("location and audit remain committed");
        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}

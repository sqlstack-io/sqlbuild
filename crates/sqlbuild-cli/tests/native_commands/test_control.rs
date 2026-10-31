use std::process::Command;

use crate::helpers::{api_ready, isolated_binary, start_control, wait_for_health};
use crate::test_types::{CommandTestCase, OpenApiTestCase, ServiceTestCase, TriggerCursorTestCase};

#[test]
fn given_a_standalone_binary_when_printing_help_or_rejecting_invalid_input_then_python_and_database_access_are_not_required()
 {
    let test_cases = [
        CommandTestCase {
            description: "native server help",
            arguments: &["server", "--help"],
            expected_success: true,
            expected_stdout_prefix: "usage: sqb ",
            expected_stderr: "",
        },
        CommandTestCase {
            description: "native scheduler help",
            arguments: &["scheduler", "--help"],
            expected_success: true,
            expected_stdout_prefix: "usage: sqb ",
            expected_stderr: "",
        },
        CommandTestCase {
            description: "native privileged administration help",
            arguments: &["admin", "--help"],
            expected_success: true,
            expected_stdout_prefix: "usage: sqb admin ",
            expected_stderr: "",
        },
        CommandTestCase {
            description: "native trigger administration help",
            arguments: &["admin", "change-trigger", "--help"],
            expected_success: true,
            expected_stdout_prefix: "usage: sqb admin change-trigger ",
            expected_stderr: "",
        },
        CommandTestCase {
            description: "native cancellation help",
            arguments: &["admin", "cancel-run", "--help"],
            expected_success: true,
            expected_stdout_prefix: "usage: sqb admin cancel-run ",
            expected_stderr: "",
        },
        CommandTestCase {
            description: "native manual job request help",
            arguments: &["admin", "request-run", "--help"],
            expected_success: true,
            expected_stdout_prefix: "usage: sqb admin request-run ",
            expected_stderr: "",
        },
        CommandTestCase {
            description: "native location work help",
            arguments: &["admin", "check-location", "--help"],
            expected_success: true,
            expected_stdout_prefix: "usage: sqb admin check-location ",
            expected_stderr: "",
        },
        CommandTestCase {
            description: "blank location work reason rejected before store initialization",
            arguments: &[
                "admin",
                "check-location",
                "00000000-0000-0000-0000-000000000001",
                "--target",
                "development",
                "--request-id",
                "00000000-0000-0000-0000-000000000002",
                "--reason",
                " ",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: target and reason must not be blank\n",
        },
        CommandTestCase {
            description: "blank job rejected before store initialization",
            arguments: &[
                "admin",
                "request-run",
                "00000000-0000-0000-0000-000000000001",
                " ",
                "--target",
                "development",
                "--run-id",
                "00000000-0000-0000-0000-000000000002",
                "--reason",
                "Refresh orders",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: target and job must not be blank\n",
        },
        CommandTestCase {
            description: "oversized manual queue priority rejected before store initialization",
            arguments: &[
                "admin",
                "request-run",
                "00000000-0000-0000-0000-000000000001",
                "daily_orders",
                "--target",
                "development",
                "--run-id",
                "00000000-0000-0000-0000-000000000002",
                "--priority",
                "9223372036854775808",
                "--reason",
                "Refresh orders",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: --priority must fit a signed 64-bit integer\n",
        },
        CommandTestCase {
            description: "blank cancellation reason rejected before store initialization",
            arguments: &[
                "admin",
                "cancel-run",
                "00000000-0000-0000-0000-000000000001",
                "--event-id",
                "00000000-0000-0000-0000-000000000002",
                "--reason",
                " ",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: --reason must not be blank\n",
        },
        CommandTestCase {
            description: "blank trigger reason rejected before store initialization",
            arguments: &[
                "admin",
                "change-trigger",
                "00000000-0000-0000-0000-000000000001",
                "pause",
                "--event-id",
                "00000000-0000-0000-0000-000000000002",
                "--reason",
                " ",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: --reason must not be blank\n",
        },
        CommandTestCase {
            description: "set-cursor requires a timestamp before store initialization",
            arguments: &[
                "admin",
                "change-trigger",
                "00000000-0000-0000-0000-000000000001",
                "set-cursor",
                "--event-id",
                "00000000-0000-0000-0000-000000000002",
                "--reason",
                "Refresh orders",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: trigger change: missing field `time`\n",
        },
        CommandTestCase {
            description: "blank location rejected before store initialization",
            arguments: &["admin", "create-location", "   "],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: location name is required\n",
        },
        CommandTestCase {
            description: "blank principal rejected before store initialization",
            arguments: &["admin", "create-principal", "   "],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: principal name is required\n",
        },
        CommandTestCase {
            description: "invalid credential ID rejected before store initialization",
            arguments: &["admin", "revoke-token", "not-a-uuid"],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: a valid subject UUID is required\n",
        },
        CommandTestCase {
            description: "invalid expiry rejected before store initialization",
            arguments: &[
                "admin",
                "issue-location-token",
                "00000000-0000-0000-0000-000000000001",
                "--expires-at",
                "tomorrow",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: --expires-at must be an RFC3339 timestamp\n",
        },
        CommandTestCase {
            description: "expired credential rejected before store initialization",
            arguments: &[
                "admin",
                "issue-location-token",
                "00000000-0000-0000-0000-000000000001",
                "--expires-at",
                "2000-01-01T00:00:00Z",
            ],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control administration failed: invalid control-plane data: token expiry must be in the future\n",
        },
        CommandTestCase {
            description: "invalid port is rejected before store initialization",
            arguments: &["server", "--port", "65536"],
            expected_success: false,
            expected_stdout_prefix: "",
            expected_stderr: "Control service failed: invalid control-plane data: --port must be between 0 and 65535\n",
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("isolated CLI directory");
        let binary = isolated_binary(directory.path());
        let output = Command::new(&binary)
            .args(test_case.arguments)
            .current_dir(directory.path())
            .env("PATH", "")
            .env("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://not-opened")
            .output()
            .expect("standalone native command");
        assert_eq!(
            output.status.success(),
            test_case.expected_success,
            "{}",
            test_case.description
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            test_case.expected_stderr,
            "{}",
            test_case.description
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).starts_with(test_case.expected_stdout_prefix),
            "{}",
            test_case.description
        );
        assert!(
            !directory.path().join(".sqlbuild").exists(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_cursor_free_trigger_change_when_a_cursor_is_supplied_then_it_fails_before_opening_the_store()
 {
    let test_cases = [
        TriggerCursorTestCase {
            description: "enable rejects an extra cursor",
            change: "enable",
            expected_success: false,
        },
        TriggerCursorTestCase {
            description: "pause rejects an extra cursor",
            change: "pause",
            expected_success: false,
        },
        TriggerCursorTestCase {
            description: "reset-default rejects an extra cursor",
            change: "reset-default",
            expected_success: false,
        },
        TriggerCursorTestCase {
            description: "reset-cursor rejects an extra cursor",
            change: "reset-cursor",
            expected_success: false,
        },
    ];
    let directory = tempfile::tempdir().expect("isolated CLI directory");
    let binary = isolated_binary(directory.path());
    for test_case in test_cases {
        for cursor in ["2026-01-01T00:00:00Z", "nonsense"] {
            let output = Command::new(&binary)
                .args([
                    "admin",
                    "change-trigger",
                    "00000000-0000-0000-0000-000000000001",
                    test_case.change,
                    "--event-id",
                    "00000000-0000-0000-0000-000000000002",
                    "--reason",
                    "Refresh orders",
                    "--cursor",
                    cursor,
                ])
                .current_dir(directory.path())
                .env("PATH", "")
                .env("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://not-opened")
                .output()
                .expect("standalone native command");
            assert_eq!(
                output.status.success(),
                test_case.expected_success,
                "{}: {cursor}",
                test_case.description
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stderr),
                "Control administration failed: invalid control-plane data: --cursor is only valid for set-cursor\n",
                "{}: {cursor}",
                test_case.description
            );
            assert!(
                output.stdout.is_empty(),
                "{}: {cursor}",
                test_case.description
            );
            assert!(!directory.path().join(".sqlbuild").exists());
        }
    }
}

#[test]
fn given_a_standalone_server_when_exporting_openapi_then_the_worker_contract_is_machine_readable_without_python_or_a_database()
 {
    let test_cases = [OpenApiTestCase {
        description: "worker lifecycle endpoint",
        expected_path: "/api/runs/{run_id}/events",
    }];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("isolated CLI directory");
        let output = Command::new(isolated_binary(directory.path()))
            .args(["server", "--openapi"])
            .current_dir(directory.path())
            .env("PATH", "")
            .env("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://not-opened")
            .output()
            .expect("standalone OpenAPI");
        assert!(output.status.success(), "{}", test_case.description);
        assert!(output.stderr.is_empty(), "{}", test_case.description);
        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("machine-readable OpenAPI");
        assert!(
            document["paths"][test_case.expected_path].is_object(),
            "{}",
            test_case.description
        );
        assert!(
            !directory.path().join(".sqlbuild").exists(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_standalone_binary_when_serving_with_or_without_a_scheduler_then_migrations_auth_and_shutdown_are_native()
 {
    let test_cases = [
        ServiceTestCase {
            description: "server without scheduling still observes platform health",
            arguments: &["server", "--port", "0", "--no-scheduler"],
            expected_leaders: 0,
            expected_heartbeats: 1,
        },
        ServiceTestCase {
            description: "server with embedded scheduling",
            arguments: &["server", "--port", "0"],
            expected_leaders: 1,
            expected_heartbeats: 1,
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(test_case.arguments);
            let url = api_ready(&mut control).await;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("bounded HTTP requests");
            let response = client
                .post(format!("{url}/api/runs/{}/heartbeat", uuid::Uuid::now_v7()))
                .send()
                .await
                .expect("protected native API");
            assert_eq!(
                response.status(),
                reqwest::StatusCode::UNAUTHORIZED,
                "{}",
                test_case.description
            );
            let (leaders, heartbeats) = wait_for_health(&mut control, &test_case).await;
            assert_eq!(
                leaders, test_case.expected_leaders,
                "{}",
                test_case.description
            );
            assert!(
                heartbeats >= test_case.expected_heartbeats,
                "{}",
                test_case.description
            );
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[test]
fn given_a_standalone_scheduler_when_started_without_python_then_it_migrates_and_takes_the_durable_lease()
 {
    let test_cases = [ServiceTestCase {
        description: "standalone scheduler takes a lease without starting a server watchdog",
        arguments: &["scheduler"],
        expected_leaders: 1,
        expected_heartbeats: 0,
    }];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(test_case.arguments);
            let (leaders, heartbeats) = wait_for_health(&mut control, &test_case).await;
            assert_eq!(
                leaders, test_case.expected_leaders,
                "{}",
                test_case.description
            );
            assert_eq!(
                heartbeats, test_case.expected_heartbeats,
                "{}",
                test_case.description
            );
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

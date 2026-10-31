use std::process::Command;
use std::time::Duration;

use crate::helpers::{api_ready, health_counts, isolated_binary, start_control};

#[test]
fn given_a_standalone_binary_when_printing_control_help_or_openapi_then_python_and_database_access_are_not_required()
 {
    let directory = tempfile::tempdir().expect("isolated CLI directory");
    let binary = isolated_binary(directory.path());
    for arguments in [
        ["server", "--help"],
        ["scheduler", "--help"],
        ["server", "--openapi"],
    ] {
        let output = Command::new(&binary)
            .args(arguments)
            .current_dir(directory.path())
            .env("PATH", "")
            .env("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://not-opened")
            .output()
            .expect("standalone native command");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        if arguments[1] == "--openapi" {
            let document: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("machine-readable OpenAPI");
            assert!(document["paths"]["/api/runs/{run_id}/events"].is_object());
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).starts_with("usage: sqb "));
        }
    }
    assert!(!directory.path().join(".sqlbuild").exists());
    let output = Command::new(&binary)
        .args(["server", "--port", "65536"])
        .current_dir(directory.path())
        .env("PATH", "")
        .output()
        .expect("invalid port");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--port must be between 0 and 65535"));
    assert!(
        !directory.path().join(".sqlbuild").exists(),
        "invalid input cannot initialize a database"
    );
}

#[tokio::test]
async fn given_a_standalone_binary_when_serving_with_or_without_a_scheduler_then_migrations_auth_and_shutdown_are_native()
 {
    for scheduler in [false, true] {
        let mut arguments = vec!["server", "--port", "0"];
        if !scheduler {
            arguments.push("--no-scheduler");
        }
        let mut control = start_control(&arguments);
        let url = api_ready(&mut control).await;
        let response = reqwest::Client::new()
            .post(format!("{url}/api/runs/{}/heartbeat", uuid::Uuid::now_v7()))
            .send()
            .await
            .expect("protected native API");
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        tokio::time::timeout(Duration::from_secs(5), async {
            while !health_counts(&control).is_some_and(|(leaders, heartbeats)| {
                leaders == i64::from(scheduler) && heartbeats > 0
            }) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("watchdog records health with or without the optional scheduler");
        #[cfg(unix)]
        crate::helpers::terminate(&mut control).await;
    }
}

#[tokio::test]
async fn given_a_standalone_scheduler_when_started_without_python_then_it_migrates_and_takes_the_durable_lease()
 {
    let mut control = start_control(&["scheduler"]);
    tokio::time::timeout(Duration::from_secs(5), async {
        while health_counts(&control).map(|(leaders, _)| leaders) != Some(1) {
            assert!(
                control
                    .child
                    .try_wait()
                    .expect("scheduler status")
                    .is_none()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("standalone scheduler acquires leadership");
    #[cfg(unix)]
    crate::helpers::terminate(&mut control).await;
}

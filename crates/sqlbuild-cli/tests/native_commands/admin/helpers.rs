//! Run the isolated native executable; never include returned bearer bytes in assertion messages.

use std::process::{Command, Output};

use serde_json::Value;
use sqlbuild_orchestration::declarations::models::TriggerRecord;
use uuid::Uuid;

use crate::test_types::TestControl;

pub(crate) fn administer(control: &TestControl, arguments: &[&str]) -> Value {
    let output = invoke_admin(control, arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Opening control store...\nControl store ready\nControl administration completed successfully\n"
    );
    serde_json::from_slice(&output.stdout).expect("JSON-only admin stdout")
}

pub(crate) fn invoke_admin(control: &TestControl, arguments: &[&str]) -> Output {
    Command::new(
        control
            .directory
            .path()
            .join(format!("sqb{}", std::env::consts::EXE_SUFFIX)),
    )
    .arg("admin")
    .args(arguments)
    .current_dir(control.directory.path())
    .env("PATH", "")
    .env_remove("SQLBUILD_CONTROL_DATABASE_URL")
    .output()
    .expect("native administration process")
}

pub(crate) async fn read_triggers(
    url: &str,
    bearer: &str,
    (location, target): (Uuid, &str),
) -> Vec<TriggerRecord> {
    let mut endpoint = reqwest::Url::parse(url).expect("API address");
    endpoint
        .path_segments_mut()
        .expect("API path")
        .pop_if_empty()
        .extend([
            "api",
            "locations",
            &location.to_string(),
            "targets",
            target,
            "triggers",
        ]);
    reqwest::Client::new()
        .get(endpoint)
        .bearer_auth(bearer)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .expect("trigger response")
        .error_for_status()
        .expect("authorized trigger read")
        .json()
        .await
        .expect("trigger records")
}

pub(crate) fn audit_actions(control: &TestControl, bearer: &str) -> Vec<String> {
    let connection =
        rusqlite::Connection::open(control.directory.path().join(".sqlbuild/control.sqlite3"))
            .expect("inspect control audit");
    let mut query = connection.prepare("SELECT action, actor, granting_role, scope_json || before_json || after_json FROM sqb_security_audit ORDER BY occurred_at, event_id")
        .expect("audit records");
    query
        .query_map([], |row| {
            let action: String = row.get(0)?;
            let actor: String = row.get(1)?;
            let role: String = row.get(2)?;
            let payload: String = row.get(3)?;
            assert_eq!(actor, "local-control-store-owner");
            assert_eq!(role, "control-store-owner");
            assert!(!payload.contains(bearer));
            Ok(action)
        })
        .expect("read audit")
        .collect::<Result<_, _>>()
        .expect("decode audit")
}

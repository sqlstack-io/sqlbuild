//! Run the isolated native executable; never include returned bearer bytes in assertion messages.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use sqlbuild_orchestration::declarations::models::TriggerRecord;
use uuid::Uuid;

use crate::test_types::TestControl;

/// The JSON result, asserting that `--json` prints nothing else.
pub(crate) fn administer(control: &TestControl, arguments: &[&str]) -> Value {
    let output = invoke_admin(control, &[arguments, &["--json"]].concat());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(without_opening_notice(&output), "");
    serde_json::from_slice(&output.stdout).expect("JSON-only admin stdout")
}

/// Stderr less the notice a slow open adds, which a loaded machine can cause.
pub(crate) fn without_opening_notice(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .replace("Opening control store...\nControl store ready\n", "")
}

pub(crate) fn invoke_admin(control: &TestControl, arguments: &[&str]) -> Output {
    admin_command(control, arguments)
        .output()
        .expect("native administration process")
}

/// `invoke_admin` with one more environment variable, such as the one `--key-from-env` names.
pub(crate) fn invoke_admin_with(
    control: &TestControl,
    arguments: &[&str],
    (name, value): (&str, &str),
) -> Output {
    admin_command(control, arguments)
        .env(name, value)
        .output()
        .expect("native administration process")
}

fn admin_command(control: &TestControl, arguments: &[&str]) -> Command {
    let mut command = Command::new(
        control
            .directory
            .path()
            .join(format!("sqb{}", std::env::consts::EXE_SUFFIX)),
    );
    command
        .arg("admin")
        .args(arguments)
        .current_dir(control.directory.path())
        .env("PATH", "")
        .env_remove("SQLBUILD_CONTROL_DATABASE_URL");
    command
}

pub(crate) fn invoke_password(control: &TestControl, principal: &str, input: &[u8]) -> Output {
    let mut child = admin_command(control, &["set-password", principal, "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("native password provisioning");
    let mut stdin = child.stdin.take().expect("password pipe");
    stdin
        .write_all(input)
        .expect("deliver password through stdin");
    drop(stdin);
    child
        .wait_with_output()
        .expect("password command completed")
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

/// The status of reading the bootstrap with `key` from an address the server doesn't name.
pub(crate) async fn keyed_bootstrap_status(url: &str, key: &str) -> u16 {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("HTTP client")
        .get(format!("{url}/api/control/bootstrap"))
        .header(reqwest::header::HOST, "10.20.30.40:8000")
        .bearer_auth(key)
        .send()
        .await
        .expect("keyed request")
        .status()
        .as_u16()
}

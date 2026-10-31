//! Run the isolated native executable; never include returned bearer bytes in assertion messages.

use std::process::Command;

use serde_json::Value;

use crate::test_types::TestControl;

pub(crate) fn administer(control: &TestControl, arguments: &[&str]) -> Value {
    let output = Command::new(
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
    .expect("native administration process");
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

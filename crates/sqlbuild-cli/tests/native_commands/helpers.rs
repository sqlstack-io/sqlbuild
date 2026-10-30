use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::test_types::TestControl;

pub(crate) fn isolated_binary(directory: &Path) -> std::path::PathBuf {
    let binary = directory.join(if cfg!(windows) { "sqb.exe" } else { "sqb" });
    std::fs::copy(env!("CARGO_BIN_EXE_sqb"), &binary)
        .expect("copy standalone binary without Python CLI");
    binary
}

pub(crate) fn start_control(arguments: &[&str]) -> TestControl {
    let directory = tempfile::tempdir().expect("isolated native control project");
    let binary = isolated_binary(directory.path());
    let stderr = directory.path().join("control.log");
    let child = Command::new(binary)
        .args(arguments)
        .current_dir(directory.path())
        .env("PATH", "")
        .env_remove("SQLBUILD_CONTROL_DATABASE_URL")
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&stderr).expect("control stderr"))
        .spawn()
        .expect("start native control process");
    TestControl {
        child,
        directory,
        stderr,
    }
}

pub(crate) async fn api_ready(control: &mut TestControl) -> String {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .expect("HTTP client");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(
                control.child.try_wait().expect("child status").is_none(),
                "control process exited: {}",
                std::fs::read_to_string(&control.stderr).unwrap_or_default()
            );
            let log = std::fs::read_to_string(&control.stderr).expect("read control log");
            if let Some(address) = log
                .lines()
                .find_map(|line| line.strip_prefix("Control API ready: "))
            {
                let url = format!("http://{address}");
                if client
                    .get(format!("{url}/api/openapi.json"))
                    .send()
                    .await
                    .is_ok_and(|response| response.status().is_success())
                {
                    return url;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("native API becomes ready")
}

pub(crate) fn leader_count(control: &TestControl) -> Option<i64> {
    let connection = rusqlite::Connection::open_with_flags(
        control.directory.path().join(".sqlbuild/control.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    connection
        .query_row("SELECT COUNT(*) FROM sqb_scheduler_lease", [], |row| {
            row.get(0)
        })
        .ok()
}

#[cfg(unix)]
pub(crate) async fn terminate(control: &mut TestControl) {
    let status = Command::new("kill")
        .args(["-TERM", &control.child.id().to_string()])
        .status()
        .expect("send SIGTERM");
    assert!(status.success());
    let status = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = control.child.try_wait().expect("child exit") {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("control roles stop promptly");
    assert!(
        status.success(),
        "shutdown failed: {}",
        std::fs::read_to_string(&control.stderr).unwrap_or_default()
    );
    assert!(
        std::fs::read_to_string(&control.stderr)
            .expect("shutdown log")
            .ends_with("Control service stopped successfully\n")
    );
}

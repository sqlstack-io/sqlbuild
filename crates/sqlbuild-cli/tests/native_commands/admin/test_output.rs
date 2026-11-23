//! sqb admin says what it did on stderr and keeps stdout for a bare secret, plain rows or `--json`.

use crate::admin_helpers::{administer, invoke_admin, without_opening_notice};
use crate::helpers::{api_ready, start_control};
use crate::test_types::AdminOutputTestCase;
#[cfg(unix)]
use crate::test_types::{AdminOpeningTestCase, SecretDeliveryTestCase};

#[test]
fn given_admin_commands_without_json_when_they_run_then_each_prints_one_line_and_stdout_only_a_secret_or_rows()
 {
    let test_cases = [
        AdminOutputTestCase {
            description: "a location",
            arguments: &["create-location", "inventory"],
            expected_stdout: ("", 0),
            expected_line: "Registered location inventory (",
        },
        AdminOutputTestCase {
            description: "a location label with a line break",
            arguments: &["create-location", "north\nwest"],
            expected_stdout: ("", 0),
            expected_line: "Registered location north\\nwest (",
        },
        AdminOutputTestCase {
            description: "a person",
            arguments: &["create-principal", "kim"],
            expected_stdout: ("", 0),
            expected_line: "Created person kim (",
        },
        AdminOutputTestCase {
            description: "a recovered admin",
            arguments: &["grant-admin", "{person}"],
            expected_stdout: ("", 0),
            expected_line: "Made sam ({person}) an admin",
        },
        AdminOutputTestCase {
            description: "an issued token alone on stdout",
            arguments: &[
                "issue-location-token",
                "{location}",
                "--expires-at",
                "2099-11-20T14:30:00Z",
            ],
            expected_stdout: ("", 1),
            expected_line: "Issued a location token for orders (",
        },
        AdminOutputTestCase {
            description: "a rotated token alone on stdout",
            arguments: &["rotate-token", "{token}"],
            expected_stdout: ("", 1),
            expected_line: "Replaced token {token} with ",
        },
        AdminOutputTestCase {
            description: "a revoked token",
            arguments: &["revoke-token", "{token}"],
            expected_stdout: ("", 0),
            expected_line: "Revoked token {token}",
        },
        AdminOutputTestCase {
            description: "a new key alone on stdout",
            arguments: &["create-service-account", "ci-runner", "--role", "developer"],
            expected_stdout: ("sqbk_", 1),
            expected_line: "Created service account ci-runner (",
        },
        AdminOutputTestCase {
            description: "a rotated key alone on stdout",
            arguments: &["rotate-service-account", "orders-deploy"],
            expected_stdout: ("sqbk_", 1),
            expected_line: "Rotated the key of service account orders-deploy (",
        },
        AdminOutputTestCase {
            description: "a disabled account",
            arguments: &["disable-service-account", "orders-deploy"],
            expected_stdout: ("", 0),
            expected_line: "Disabled service account orders-deploy (",
        },
        AdminOutputTestCase {
            description: "an enabled account",
            arguments: &["enable-service-account", "orders-deploy"],
            expected_stdout: ("", 0),
            expected_line: "Enabled service account orders-deploy (",
        },
        AdminOutputTestCase {
            description: "a list as plain rows",
            arguments: &["list-service-accounts"],
            expected_stdout: ("orders-deploy", 1),
            expected_line: "",
        },
        AdminOutputTestCase {
            description: "an absent location request",
            arguments: &["location-work", "{person}"],
            expected_stdout: ("", 0),
            expected_line: "No location request {person}",
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
            api_ready(&mut control).await;
            let field = |value: &serde_json::Value| value.as_str().expect("identity").to_owned();
            let location =
                field(&administer(&control, &["create-location", "orders"])["location_id"]);
            let expiry = "2099-01-01T00:00:00Z";
            let issued = administer(
                &control,
                &["issue-location-token", &location, "--expires-at", expiry],
            );
            let token = field(&issued["identity"]["token_id"]);
            let person = field(&administer(&control, &["create-principal", "sam"])["principal_id"]);
            let account = [
                "create-service-account",
                "orders-deploy",
                "--role",
                "viewer",
            ];
            administer(&control, &account);
            let fill = |text: &str| {
                text.replace("{location}", &location)
                    .replace("{person}", &person)
                    .replace("{token}", &token)
            };
            let arguments: Vec<String> =
                test_case.arguments.iter().map(|text| fill(text)).collect();
            let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
            let output = invoke_admin(&control, &arguments);
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = without_opening_notice(&output);
            let expected = fill(test_case.expected_line);
            let secret = stdout.trim_end();
            assert_eq!(
                (
                    output.status.success(),
                    stdout.starts_with(test_case.expected_stdout.0),
                    stdout.lines().count(),
                    stderr.starts_with(&expected),
                    stderr.lines().count(),
                    secret.is_empty() || !stderr.contains(secret),
                ),
                (
                    true,
                    true,
                    test_case.expected_stdout.1,
                    true,
                    usize::from(!expected.is_empty()),
                    true,
                ),
                "{}: {stderr}",
                test_case.description
            );
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[cfg(unix)]
#[test]
fn given_a_verbose_or_slow_open_when_sqb_admin_runs_then_it_says_the_store_opens() {
    let test_cases = [
        AdminOpeningTestCase {
            description: "--verbose",
            arguments: &["admin", "create-location", "orders", "--verbose"],
            held_milliseconds: 0,
            expected_stdout_empty: true,
            expected_stderr_prefix: "Opening control store...\nControl store ready\nRegistered location orders (",
        },
        AdminOpeningTestCase {
            description: "a store that takes noticeably long to open",
            arguments: &["admin", "create-location", "orders"],
            held_milliseconds: 2500,
            expected_stdout_empty: true,
            expected_stderr_prefix: "Opening control store...\nControl store ready\nRegistered location orders (",
        },
        AdminOpeningTestCase {
            description: "--json with --verbose keeps stdout to the result",
            arguments: &["admin", "create-location", "orders", "--json", "-v"],
            held_milliseconds: 0,
            expected_stdout_empty: false,
            expected_stderr_prefix: "Opening control store...\nControl store ready\n",
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("installation");
        let binary = crate::helpers::isolated_binary(directory.path());
        let state = directory.path().join(".sqlbuild");
        std::fs::create_dir(&state).expect("state directory");
        let writer = rusqlite::Connection::open(state.join("control.sqlite3")).expect("store");
        writer
            .execute_batch("BEGIN EXCLUSIVE")
            .expect("hold the store");
        let child = std::process::Command::new(binary)
            .args(test_case.arguments)
            .current_dir(directory.path())
            .env("PATH", "")
            .env_remove("SQLBUILD_CONTROL_DATABASE_URL")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("native administration");
        std::thread::sleep(std::time::Duration::from_millis(
            test_case.held_milliseconds,
        ));
        writer.execute_batch("ROLLBACK").expect("release the store");
        let output = child.wait_with_output().expect("administration output");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert_eq!(
            (
                output.status.success(),
                output.stdout.is_empty(),
                stderr.starts_with(test_case.expected_stderr_prefix),
            ),
            (true, test_case.expected_stdout_empty, true),
            "{}: {stderr}",
            test_case.description
        );
    }
}

#[cfg(unix)]
#[test]
fn given_an_issued_secret_when_stdout_fails_then_it_fails_without_printing_the_line_or_the_secret()
{
    let test_cases = [SecretDeliveryTestCase {
        description: "a new service account key to a full device",
        expected_exit: 1,
        expected_stderr: "Control administration result delivery failed; changes may already be committed\n",
    }];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("installation");
        let output = std::process::Command::new(crate::helpers::isolated_binary(directory.path()))
            .args([
                "admin",
                "create-service-account",
                "orders-deploy",
                "--role",
                "viewer",
            ])
            .current_dir(directory.path())
            .env("PATH", "")
            .env_remove("SQLBUILD_CONTROL_DATABASE_URL")
            .stdout(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .expect("full device"),
            )
            .output()
            .expect("native administration");
        assert_eq!(
            (
                output.status.code(),
                without_opening_notice(&output).as_str()
            ),
            (Some(test_case.expected_exit), test_case.expected_stderr),
            "{}",
            test_case.description
        );
    }
}

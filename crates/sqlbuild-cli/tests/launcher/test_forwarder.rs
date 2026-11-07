//! `sqlbuild` behaves exactly like the `sqb` installed beside it and fails loudly without one.

use std::io::Write;
use std::process::{Command, Stdio};

#[cfg(windows)]
use crate::helpers::break_forwarder;
#[cfg(unix)]
use crate::helpers::ready_pid;
use crate::helpers::{forwarder_in, forwarder_with_fake_sqb, sqb_file};
#[cfg(windows)]
use crate::test_types::ForwardedBreakTestCase;
#[cfg(unix)]
use crate::test_types::ForwardedSignalTestCase;
use crate::test_types::{
    ForwardedArgumentsTestCase, ForwardedExitTestCase, ForwardedStreamsTestCase, MissingSqbTestCase,
};

#[test]
fn given_arguments_when_running_sqlbuild_then_sqb_receives_them_unchanged() {
    let test_cases = [
        ForwardedArgumentsTestCase {
            description: "no arguments",
            arguments: &[],
            expected_lines: &[],
        },
        ForwardedArgumentsTestCase {
            description: "spaces, quotes, unicode, empty and dash-leading arguments",
            arguments: &[
                "--project-dir",
                "orders shop",
                "say \"hi\"",
                "commandé",
                "",
                "--",
                "-x",
            ],
            expected_lines: &[
                r#"arg:"--project-dir""#,
                r#"arg:"orders shop""#,
                r#"arg:"say \"hi\"""#,
                r#"arg:"commandé""#,
                r#"arg:"""#,
                r#"arg:"--""#,
                r#"arg:"-x""#,
            ],
        },
    ];
    let directory = tempfile::tempdir().expect("isolated install");
    let forwarder = forwarder_with_fake_sqb(directory.path());
    for test_case in test_cases {
        let output = Command::new(&forwarder)
            .args(test_case.arguments)
            .output()
            .expect("run sqlbuild");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
        assert_eq!(
            stdout.lines().collect::<Vec<_>>(),
            test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sqb_exit_code_when_running_sqlbuild_then_sqlbuild_exits_with_the_same_code() {
    let test_cases = [
        ForwardedExitTestCase {
            description: "success",
            exit: 0,
            expected_code: 0,
        },
        ForwardedExitTestCase {
            description: "usage error",
            exit: 2,
            expected_code: 2,
        },
        ForwardedExitTestCase {
            description: "largest portable code",
            exit: 255,
            expected_code: 255,
        },
        #[cfg(windows)]
        ForwardedExitTestCase {
            description: "Windows status beyond one byte",
            exit: -1_073_741_510,
            expected_code: -1_073_741_510,
        },
    ];
    let directory = tempfile::tempdir().expect("isolated install");
    let forwarder = forwarder_with_fake_sqb(directory.path());
    for test_case in test_cases {
        let status = Command::new(&forwarder)
            .env("FAKE_EXIT", test_case.exit.to_string())
            .stderr(Stdio::null())
            .status()
            .expect("run sqlbuild");
        assert_eq!(
            status.code(),
            Some(test_case.expected_code),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_input_when_running_sqlbuild_then_sqb_shares_stdin_stdout_and_stderr() {
    let test_cases = [
        ForwardedStreamsTestCase {
            description: "multi-line input",
            stdin: "orders\nline two\n",
            expected_stdout: "stdin:orders\nline two\n",
            expected_stderr: "fake stderr",
        },
        ForwardedStreamsTestCase {
            description: "empty input",
            stdin: "",
            expected_stdout: "stdin:",
            expected_stderr: "fake stderr",
        },
    ];
    let directory = tempfile::tempdir().expect("isolated install");
    let forwarder = forwarder_with_fake_sqb(directory.path());
    for test_case in test_cases {
        let mut child = Command::new(&forwarder)
            .env("FAKE_STDIN", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run sqlbuild");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(test_case.stdin.as_bytes())
            .expect("write stdin");
        let output = child.wait_with_output().expect("sqlbuild output");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            test_case.expected_stdout,
            "{}",
            test_case.description
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            test_case.expected_stderr,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_no_sqb_beside_it_when_running_sqlbuild_then_it_fails_and_names_the_missing_program() {
    let test_cases = [
        MissingSqbTestCase {
            description: "version request",
            arguments: &["--version"],
            expected_code: Some(1),
            expected_stdout: "",
            expected_stderr_start: format!("sqlbuild: cannot find {} beside ", sqb_file()),
        },
        MissingSqbTestCase {
            description: "command",
            arguments: &["build", "--select", "orders"],
            expected_code: Some(1),
            expected_stdout: "",
            expected_stderr_start: format!("sqlbuild: cannot find {} beside ", sqb_file()),
        },
    ];
    let directory = tempfile::tempdir().expect("isolated install");
    let forwarder = forwarder_in(directory.path());
    for test_case in test_cases {
        let output = Command::new(&forwarder)
            .args(test_case.arguments)
            .output()
            .expect("run sqlbuild");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            test_case.expected_code,
            "{}",
            test_case.description
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            test_case.expected_stdout,
            "{}",
            test_case.description
        );
        assert!(
            stderr.starts_with(&test_case.expected_stderr_start),
            "{}: {stderr}",
            test_case.description
        );
    }
}

#[cfg(unix)]
#[test]
fn given_a_signal_when_sqb_runs_through_sqlbuild_then_sqb_is_the_same_process_and_receives_it() {
    use std::os::unix::process::ExitStatusExt;

    let test_cases = [
        ForwardedSignalTestCase {
            description: "termination",
            signal: "TERM",
            expected_signal: Some(15),
        },
        ForwardedSignalTestCase {
            description: "interrupt",
            signal: "INT",
            expected_signal: Some(2),
        },
    ];
    let directory = tempfile::tempdir().expect("isolated install");
    let forwarder = forwarder_with_fake_sqb(directory.path());
    for test_case in test_cases {
        let mut child = Command::new(&forwarder)
            .env("FAKE_WAIT", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("run sqlbuild");
        let pid = ready_pid(&mut child);
        assert_eq!(pid, child.id(), "{}", test_case.description);
        let sent = Command::new("kill")
            .args([format!("-{}", test_case.signal), pid.to_string()])
            .status()
            .expect("send the signal");
        assert!(sent.success(), "{}", test_case.description);
        assert_eq!(
            child.wait().expect("wait").signal(),
            test_case.expected_signal,
            "{}",
            test_case.description
        );
    }
}

#[cfg(windows)]
#[test]
fn given_ctrl_break_when_sqb_runs_through_sqlbuild_then_only_sqb_handles_it_and_its_code_returns() {
    let test_cases = [ForwardedBreakTestCase {
        description: "sqb exits 42 half a second after Ctrl+Break while sqlbuild waits",
        expected_result: "1 42",
    }];
    let directory = tempfile::tempdir().expect("isolated install");
    let forwarder = forwarder_with_fake_sqb(directory.path());
    for test_case in test_cases {
        assert_eq!(
            break_forwarder(directory.path(), &forwarder),
            test_case.expected_result,
            "{}",
            test_case.description
        );
    }
}

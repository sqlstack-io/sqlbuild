use std::io;

use crate::argv::models::Stream;
use crate::launcher::constants::{PROGRAM_NAME, PROGRAM_VERSION};
use crate::native_commands::_helpers::command::terminal::{line_reset, lost_stdout};
use crate::native_commands::models::LostStdout;
use crate::native_commands::tests::helpers::parse;
use crate::native_commands::tests::test_types::{
    LineResetTestCase, LostStdoutTestCase, ParseTestCase,
};

/// Thresholds recorded from CPython 3.12 writing to a pipe without a reader.
#[test]
fn given_lost_stdout_bytes_when_classifying_then_python_buffer_outcome_matches() {
    let test_cases = [
        LostStdoutTestCase {
            description: "half the buffer stays until the final flush",
            total: 4096,
            expected_lost: LostStdout::AtExit("BrokenPipeError: [Errno 32] Broken pipe".to_owned()),
        },
        LostStdoutTestCase {
            description: "more than half is dropped by the failed flush",
            total: 4097,
            expected_lost: LostStdout::Dropped,
        },
        LostStdoutTestCase {
            description: "a full buffer fails the write",
            total: 8192,
            expected_lost: LostStdout::Raised("BrokenPipeError: [Errno 32] Broken pipe".to_owned()),
        },
    ];

    for test_case in test_cases {
        let error: io::Error = io::Error::from(io::ErrorKind::BrokenPipe);

        assert_eq!(
            lost_stdout(test_case.total, &error),
            test_case.expected_lost,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_version_among_options_when_parsing_natively_then_the_lone_version_line_prints() {
    let test_cases = [
        ParseTestCase {
            description: "after --no-color",
            argv: &["--no-color", "--version"],
            expected_needs_python: false,
        },
        ParseTestCase {
            description: "the short option after --debug",
            argv: &["--debug", "-V"],
            expected_needs_python: false,
        },
    ];

    for test_case in test_cases {
        let (outcome, host) = parse(test_case.argv);

        assert_eq!(
            (outcome.writes, outcome.parsed, host.needs_python),
            (
                vec![(
                    Stream::Stdout,
                    format!("{PROGRAM_NAME} {PROGRAM_VERSION}\n")
                )],
                Err(0),
                test_case.expected_needs_python,
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_child_spinner_on_the_terminal_when_writing_then_its_line_is_erased_first() {
    let test_cases = [
        LineResetTestCase {
            description: "a terminal shared with a location interpreter",
            shared: true,
            terminal: true,
            expected_reset: b"\r\x1b[K",
        },
        LineResetTestCase {
            description: "a log file shared with the interpreter keeps its bytes",
            shared: true,
            terminal: false,
            expected_reset: b"",
        },
        LineResetTestCase {
            description: "a terminal no child draws on",
            shared: false,
            terminal: true,
            expected_reset: b"",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            line_reset(test_case.shared, test_case.terminal),
            test_case.expected_reset,
            "{}",
            test_case.description
        );
    }
}

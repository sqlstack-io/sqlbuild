//! Admin results that need a published project to produce still name what was done.

use sqlbuild_orchestration::declarations::models::TriggerChange;
use uuid::Uuid;

use super::test_types::AdminTextTestCase;
use crate::native_commands::_helpers::admin::{rows::stdout_text, summaries::summary_line};
use crate::native_commands::models::AdminDone;

#[test]
fn given_an_admin_result_when_printed_without_json_then_one_line_names_what_was_done() {
    let [run, source, event] = [10, 11, 12].map(Uuid::from_u128);
    let test_cases = [
        AdminTextTestCase {
            description: "a queued job run",
            done: AdminDone::RunQueued {
                run_id: run,
                job: "nightly_job".to_owned(),
                target: "production".to_owned(),
                applied: true,
            },
            expected_line: Some(
                "Queued a run of nightly_job on target production (00000000-0000-0000-0000-00000000000a)",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a repeated job run",
            done: AdminDone::RunQueued {
                run_id: run,
                job: "nightly_job".to_owned(),
                target: "production".to_owned(),
                applied: false,
            },
            expected_line: Some(
                "Run 00000000-0000-0000-0000-00000000000a was already requested; nothing changed",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a queued rerun",
            done: AdminDone::RerunQueued {
                run_id: run,
                source,
                applied: true,
            },
            expected_line: Some(
                "Queued a rerun of run 00000000-0000-0000-0000-00000000000b (00000000-0000-0000-0000-00000000000a)",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a repeated rerun",
            done: AdminDone::RerunQueued {
                run_id: run,
                source,
                applied: false,
            },
            expected_line: Some(
                "Run 00000000-0000-0000-0000-00000000000a was already requested; nothing changed",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a connection check",
            done: AdminDone::CheckRequested {
                request_id: run,
                location: "orders".to_owned(),
                target: "production".to_owned(),
                applied: true,
            },
            expected_line: Some(
                "Requested a connection check of orders on target production (00000000-0000-0000-0000-00000000000a)",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a repeated check",
            done: AdminDone::CheckRequested {
                request_id: run,
                location: "orders".to_owned(),
                target: "production".to_owned(),
                applied: false,
            },
            expected_line: Some(
                "Connection check 00000000-0000-0000-0000-00000000000a was already requested; nothing changed",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a cancellation",
            done: AdminDone::CancelRequested {
                event_id: event,
                run_id: run,
                applied: true,
            },
            expected_line: Some(
                "Requested cancellation of run 00000000-0000-0000-0000-00000000000a (00000000-0000-0000-0000-00000000000c)",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a repeated cancellation",
            done: AdminDone::CancelRequested {
                event_id: event,
                run_id: run,
                applied: false,
            },
            expected_line: Some(
                "Cancellation of run 00000000-0000-0000-0000-00000000000a (00000000-0000-0000-0000-00000000000c) was already requested; nothing changed",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "an enabled trigger",
            done: AdminDone::TriggerChanged {
                event_id: event,
                trigger_id: run,
                change: TriggerChange::Enable,
                applied: true,
            },
            expected_line: Some("Enabled trigger 00000000-0000-0000-0000-00000000000a"),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a paused trigger",
            done: AdminDone::TriggerChanged {
                event_id: event,
                trigger_id: run,
                change: TriggerChange::Pause,
                applied: true,
            },
            expected_line: Some("Paused trigger 00000000-0000-0000-0000-00000000000a"),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a default status",
            done: AdminDone::TriggerChanged {
                event_id: event,
                trigger_id: run,
                change: TriggerChange::ResetDefault,
                applied: true,
            },
            expected_line: Some(
                "Reset trigger 00000000-0000-0000-0000-00000000000a to its default status",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a set cursor",
            done: AdminDone::TriggerChanged {
                event_id: event,
                trigger_id: run,
                change: TriggerChange::SetCursor {
                    time: chrono::DateTime::UNIX_EPOCH,
                },
                applied: true,
            },
            expected_line: Some(
                "Set the cursor of trigger 00000000-0000-0000-0000-00000000000a to 1970-01-01T00:00:00Z",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a reset cursor",
            done: AdminDone::TriggerChanged {
                event_id: event,
                trigger_id: run,
                change: TriggerChange::ResetCursor,
                applied: true,
            },
            expected_line: Some("Reset the cursor of trigger 00000000-0000-0000-0000-00000000000a"),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a repeated change",
            done: AdminDone::TriggerChanged {
                event_id: event,
                trigger_id: run,
                change: TriggerChange::Pause,
                applied: false,
            },
            expected_line: Some(
                "The change to trigger 00000000-0000-0000-0000-00000000000a (00000000-0000-0000-0000-00000000000c) was already made; nothing changed",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "a token revoked before",
            done: AdminDone::TokenRevoked {
                id: run,
                revoked: false,
            },
            expected_line: Some(
                "Token 00000000-0000-0000-0000-00000000000a was already revoked or doesn't exist; nothing changed",
            ),
            expected_stdout: "",
        },
        AdminTextTestCase {
            description: "no service accounts",
            done: AdminDone::AccountsListed(Vec::new()),
            expected_line: Some("No service accounts"),
            expected_stdout: "",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            (summary_line(&test_case.done), stdout_text(&test_case.done)),
            (
                test_case.expected_line.map(str::to_owned),
                test_case.expected_stdout.to_owned(),
            ),
            "{}",
            test_case.description
        );
    }
}

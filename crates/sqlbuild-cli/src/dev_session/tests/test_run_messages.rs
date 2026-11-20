//! Run event lines: one when a run starts, one when it ends, oldest first, never repeated.

use sqlbuild_orchestration::runs::models::RunStatus;

use crate::dev_session::_helpers::runs::run_messages;
use crate::dev_session::tests::helpers::{message_text, run, seen};
use crate::dev_session::tests::test_types::RunPollTestCase;

#[test]
fn given_runs_since_the_last_poll_when_read_then_each_start_and_end_is_reported_once() {
    let test_cases = [
        RunPollTestCase {
            description: "the first read only records the runs that already exist",
            seen: None,
            runs: vec![
                run(
                    2,
                    ("running", "schedule"),
                    (Some("orders_daily"), None),
                    (5, 3),
                ),
                run(1, ("success", "manual"), (Some("customers"), None), (40, 2)),
            ],
            expected_messages: Vec::new(),
        },
        RunPollTestCase {
            description: "new runs start oldest first; a run that started and ended between polls reports both",
            seen: seen(&[]),
            runs: vec![
                run(
                    3,
                    ("running", "schedule"),
                    (Some("orders_daily"), None),
                    (5, 3),
                ),
                run(
                    2,
                    ("failed", "sensor"),
                    (None, Some("new_orders_file")),
                    (12, 4),
                ),
            ],
            expected_messages: vec![
                "new_orders_file | started by sensor | running".to_owned(),
                "new_orders_file | failed \u{b7} 4 nodes | failed 12s".to_owned(),
                "orders_daily | started by schedule | running".to_owned(),
            ],
        },
        RunPollTestCase {
            description: "a seen run reports only its end, named by its kind when it has no job or trigger",
            seen: seen(&[(3, RunStatus::Running), (4, RunStatus::Running)]),
            runs: vec![
                run(4, ("warn", "manual"), (None, None), (90, 5)),
                run(
                    3,
                    ("running", "schedule"),
                    (Some("orders_daily"), None),
                    (5, 3),
                ),
            ],
            expected_messages: vec!["adhoc | succeeded \u{b7} 5 nodes | ok 90s".to_owned()],
        },
        RunPollTestCase {
            description: "a run already seen ending is not reported again",
            seen: seen(&[(4, RunStatus::Warn)]),
            runs: vec![run(4, ("warn", "manual"), (None, None), (90, 5))],
            expected_messages: Vec::new(),
        },
    ];

    for test_case in test_cases {
        let (messages, _) = run_messages(test_case.seen, &test_case.runs);

        assert_eq!(
            messages.iter().map(message_text).collect::<Vec<_>>(),
            test_case.expected_messages,
            "{}",
            test_case.description
        );
    }
}

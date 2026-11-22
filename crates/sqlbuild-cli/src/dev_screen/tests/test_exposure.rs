//! A session open beyond this machine says who gets in, at start-up and in the ready block.

use crate::dev_screen::tests::helpers::{opened, play, still_running};
use crate::dev_screen::tests::test_types::ExposureTestCase;

#[test]
fn given_a_session_open_beyond_this_machine_when_it_starts_then_the_log_and_ready_block_say_who_gets_in()
 {
    let test_cases = [
        ExposureTestCase {
            description: "live",
            size: Some((80, 24)),
            expected_lines: vec![
                "\u{26a0} No sign-in",
                "anyone who can reach 0.0.0.0:8640",
                "\u{2502} Access     anyone who can reach 0.0.0.0:8640",
            ],
        },
        ExposureTestCase {
            description: "plain",
            size: None,
            expected_lines: vec![
                "\u{26a0} No sign-in",
                "anyone who can reach 0.0.0.0:8640",
                "  access     anyone who can reach 0.0.0.0:8640",
            ],
        },
    ];
    for test_case in test_cases {
        let shown: String = play(test_case.size, opened(), still_running)
            .scrollback
            .join("\n");
        assert!(
            test_case
                .expected_lines
                .iter()
                .all(|line| shown.contains(line))
                && shown.contains("credentials"),
            "{}: {shown}",
            test_case.description
        );
    }
}

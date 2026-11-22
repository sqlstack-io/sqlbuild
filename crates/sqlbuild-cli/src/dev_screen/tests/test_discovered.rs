//! A location's discovered sets are a row of the ready box.

use crate::dev_screen::tests::helpers::{discovered, play, still_running};
use crate::dev_screen::tests::test_types::DiscoveredRowTestCase;

#[test]
fn given_discovered_sets_when_the_session_is_ready_then_the_box_has_their_row() {
    let test_cases = [DiscoveredRowTestCase {
        description: "two sets, one failing",
        summary: "orders_plan 27 items \u{b7} returns_plan \u{2717} ssh timeout (using last good, 11 items)",
        expected_lines: vec![
            "\u{2502} Discovered orders_plan 27 items",
            "\u{2502}            returns_plan \u{2717} ssh timeout (using last good, 11 items)",
        ],
    }];
    for test_case in test_cases {
        let shown: String = play(Some((80, 24)), discovered(test_case.summary), still_running)
            .scrollback
            .join("\n");
        assert!(
            test_case
                .expected_lines
                .iter()
                .all(|line| shown.contains(line)),
            "{}: {shown}",
            test_case.description
        );
    }
}

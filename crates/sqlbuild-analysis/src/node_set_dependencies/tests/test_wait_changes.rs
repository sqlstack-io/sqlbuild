use crate::node_set_dependencies::main::wait_changes::wait_changes;
use crate::node_set_dependencies::tests::helpers::recorded;
use crate::node_set_dependencies::tests::test_types::WaitChangesTestCase;

const BASE: &str = r#"{"dependencies":[]}"#;

#[test]
fn given_set_ends_that_changed_since_the_last_build_when_planning_then_lists_each_wait_change() {
    let test_cases = [
        WaitChangesTestCase {
            description: "a job gaining an end node",
            previous: Some(&[("job 'nightly_job'", &["model 'orders'"])]),
            current: &[("job 'nightly_job'", &["model 'orders'", "model 'items'"])],
            expected_lines: &["report now also waits on model 'items' through job 'nightly_job'"],
        },
        WaitChangesTestCase {
            description: "a job losing an end node",
            previous: Some(&[("job 'nightly_job'", &["model 'orders'", "model 'items'"])]),
            current: &[("job 'nightly_job'", &["model 'orders'"])],
            expected_lines: &["report no longer waits on model 'items' through job 'nightly_job'"],
        },
        WaitChangesTestCase {
            description: "unchanged end nodes",
            previous: Some(&[("tag 'finance'", &["model 'orders'"])]),
            current: &[("tag 'finance'", &["model 'orders'"])],
            expected_lines: &[],
        },
        WaitChangesTestCase {
            description: "a build that recorded no sets",
            previous: None,
            current: &[("tag 'finance'", &["model 'orders'"])],
            expected_lines: &[],
        },
    ];
    for test_case in test_cases {
        let previous: String = test_case
            .previous
            .map_or_else(|| BASE.to_owned(), |sets| recorded(BASE, sets));
        assert_eq!(
            wait_changes(
                "report",
                Some(&previous),
                Some(&recorded(BASE, test_case.current))
            ),
            test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

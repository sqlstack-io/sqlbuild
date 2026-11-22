use serde_json::json;

use crate::discovered_sets::_helpers::digest::discovered_digest;
use crate::discovered_sets::_helpers::problems::discovered_problem;
use crate::discovered_sets::main::discovery_health::discovery_health;
use crate::discovered_sets::main::discovery_summary::discovery_summary;
use crate::discovered_sets::main::item_roots::item_roots;
use crate::discovered_sets::main::parse_check_every::parse_check_every;
use crate::discovered_sets::main::record_attempt::record_attempt;
use crate::discovered_sets::main::result_changed::result_changed;
use crate::discovered_sets::models::DiscoveredSetStatus;
use crate::discovered_sets::tests::helpers::{ids, november, set};
use crate::discovered_sets::tests::test_types::{
    AttemptTestCase, CheckEveryTestCase, HealthTestCase, ProblemTestCase, RootsTestCase,
    SummaryTestCase,
};
use crate::discovered_sets::types::DiscoveredSets;

const LONG_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn given_a_discover_result_when_checking_it_then_only_a_valid_set_passes() {
    let test_cases = [
        ProblemTestCase {
            description: "a chain of items with dotted, colon and dash IDs",
            result: set(&[("load.orders", &[]), ("fit:orders-1", &["load.orders"])]),
            expected_problem: None,
        },
        ProblemTestCase {
            description: "no items",
            result: set(&[]),
            expected_problem: Some("the discover function returned no items".to_owned()),
        },
        ProblemTestCase {
            description: "an ID with a bracket",
            result: set(&[("a[1]", &[])]),
            expected_problem: Some(
                "item ID 'a[1]' uses '['; IDs use only letters, digits and _ . : -".to_owned(),
            ),
        },
        ProblemTestCase {
            description: "an empty ID",
            result: set(&[("", &[])]),
            expected_problem: Some("an item ID is empty".to_owned()),
        },
        ProblemTestCase {
            description: "an ID of 201 characters",
            result: set(&[(LONG_ID, &[])]),
            expected_problem: Some(format!("item ID '{LONG_ID}' is longer than 200 characters")),
        },
        ProblemTestCase {
            description: "a repeated ID",
            result: set(&[("a", &[]), ("a", &[])]),
            expected_problem: Some("item ID 'a' appears more than once".to_owned()),
        },
        ProblemTestCase {
            description: "a dependency on an unknown item",
            result: set(&[("a", &["b"])]),
            expected_problem: Some(
                "item 'a' depends on 'b', which is not an item of the set".to_owned(),
            ),
        },
        ProblemTestCase {
            description: "an item depending on itself",
            result: set(&[("a", &["a"])]),
            expected_problem: Some("item 'a' depends on itself".to_owned()),
        },
        ProblemTestCase {
            description: "a cycle",
            result: set(&[("a", &["b"]), ("b", &["a"]), ("c", &[])]),
            expected_problem: Some("item 'a' is on a dependency cycle".to_owned()),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            discovered_problem(&test_case.result),
            test_case.expected_problem,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_check_every_text_when_parsing_it_then_units_add_up_from_a_minute() {
    let test_cases = [
        CheckEveryTestCase {
            description: "minutes",
            text: "15m",
            expected_seconds: Some(900),
        },
        CheckEveryTestCase {
            description: "hours and minutes",
            text: "1h30m",
            expected_seconds: Some(5_400),
        },
        CheckEveryTestCase {
            description: "a day",
            text: "1d",
            expected_seconds: Some(86_400),
        },
        CheckEveryTestCase {
            description: "under a minute",
            text: "30s",
            expected_seconds: None,
        },
        CheckEveryTestCase {
            description: "units out of order",
            text: "5m1h",
            expected_seconds: None,
        },
        CheckEveryTestCase {
            description: "no unit",
            text: "15",
            expected_seconds: None,
        },
        CheckEveryTestCase {
            description: "an unknown unit",
            text: "2w",
            expected_seconds: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_check_every(test_case.text).ok(),
            test_case.expected_seconds,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_set_that_succeeded_when_an_attempt_follows_then_a_failure_keeps_the_last_good_items() {
    let test_cases = [
        AttemptTestCase {
            description: "a success without items keeps the last good ones",
            attempt: Ok(set(&[])),
            expected_ids: vec!["a", "b"],
            expected_status: DiscoveredSetStatus::Failed,
            expected_error: Some("the discover function returned no items"),
        },
        AttemptTestCase {
            description: "a success replaces the items",
            attempt: Ok(set(&[("c", &[])])),
            expected_ids: vec!["c"],
            expected_status: DiscoveredSetStatus::Ok,
            expected_error: None,
        },
        AttemptTestCase {
            description: "an error keeps the last good items",
            attempt: Err("ssh timeout".to_owned()),
            expected_ids: vec!["a", "b"],
            expected_status: DiscoveredSetStatus::Failed,
            expected_error: Some("ssh timeout"),
        },
        AttemptTestCase {
            description: "an invalid result fails like an error",
            attempt: Ok(set(&[("x", &["y"])])),
            expected_ids: vec!["a", "b"],
            expected_status: DiscoveredSetStatus::Failed,
            expected_error: Some("item 'x' depends on 'y', which is not an item of the set"),
        },
    ];
    let previous = record_attempt(
        None,
        Ok(set(&[("a", &[]), ("b", &["a"])])),
        (november(20), None),
    );

    for test_case in test_cases {
        let next = record_attempt(
            Some(&previous),
            test_case.attempt,
            (november(21), Some(900)),
        );

        assert_eq!(
            (
                ids(next.result.as_ref()),
                next.status,
                next.error.as_deref()
            ),
            (
                test_case
                    .expected_ids
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect(),
                test_case.expected_status,
                test_case.expected_error
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_set_when_reading_its_roots_and_digest_then_roots_wait_on_no_item_and_tag_order_does_not_matter()
 {
    let test_cases = [RootsTestCase {
        description: "two roots, and the same tags in another order",
        result: set(&[("a", &[]), ("b", &["a"]), ("c", &[])]),
        expected_roots: vec!["a", "c"],
        expected_same_digest: true,
    }];

    for test_case in test_cases {
        let mut first = test_case.result.clone();
        let mut second = test_case.result.clone();
        first.tags = [
            ("release".to_owned(), json!("r1")),
            ("plan".to_owned(), json!({"x": 1, "y": 2})),
        ]
        .into();
        second.tags = [
            ("plan".to_owned(), json!({"y": 2, "x": 1})),
            ("release".to_owned(), json!("r1")),
        ]
        .into();
        let same: bool = discovered_digest(&first).expect("digest")
            == discovered_digest(&second).expect("digest");

        assert_eq!(
            (item_roots(&first).into_iter().collect::<Vec<_>>(), same),
            (
                test_case
                    .expected_roots
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect(),
                test_case.expected_same_digest
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_attempt_after_a_success_when_summarising_then_failures_name_the_error_and_the_last_good_items()
 {
    let test_cases = [
        SummaryTestCase {
            description: "the same items again",
            attempt: Ok(set(&[("a", &[]), ("b", &["a"])])),
            expected_summary: "orders_plan 2 items",
            expected_changed: false,
        },
        SummaryTestCase {
            description: "a new item",
            attempt: Ok(set(&[("a", &[]), ("b", &["a"]), ("c", &[])])),
            expected_summary: "orders_plan 3 items",
            expected_changed: true,
        },
        SummaryTestCase {
            description: "a failure keeps the last good items",
            attempt: Err("ssh timeout".to_owned()),
            expected_summary: "orders_plan \u{2717} ssh timeout (using last good, 2 items)",
            expected_changed: false,
        },
    ];
    let previous = record_attempt(
        None,
        Ok(set(&[("a", &[]), ("b", &["a"])])),
        (november(20), None),
    );

    for test_case in test_cases {
        let next = record_attempt(Some(&previous), test_case.attempt, (november(21), None));
        let changed = result_changed(Some(&previous), &next);
        let sets: DiscoveredSets = [("orders_plan".to_owned(), next)].into();

        assert_eq!(
            (discovery_summary(&sets).as_deref(), changed),
            (Some(test_case.expected_summary), test_case.expected_changed),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_first_attempt_when_reporting_health_then_a_set_that_never_succeeded_serves_no_items() {
    let test_cases = [
        HealthTestCase {
            description: "a failure with no last good result",
            attempt: Err("no route".to_owned()),
            expected_health: (DiscoveredSetStatus::Failed, 0, Some("no route")),
            expected_summary: "returns_plan \u{2717} no route (never discovered)",
        },
        HealthTestCase {
            description: "a success",
            attempt: Ok(set(&[("a", &[])])),
            expected_health: (DiscoveredSetStatus::Ok, 1, None),
            expected_summary: "returns_plan 1 item",
        },
    ];

    for test_case in test_cases {
        let first = record_attempt(None, test_case.attempt, (november(21), Some(900)));
        let sets: DiscoveredSets = [("returns_plan".to_owned(), first)].into();
        let health: Vec<_> = discovery_health(&sets)
            .into_iter()
            .map(|set| (set.status, set.items, set.error))
            .collect();

        assert_eq!(
            (health, discovery_summary(&sets)),
            (
                vec![(
                    test_case.expected_health.0,
                    test_case.expected_health.1,
                    test_case.expected_health.2.map(str::to_owned)
                )],
                Some(test_case.expected_summary.to_owned())
            ),
            "{}",
            test_case.description
        );
    }
}

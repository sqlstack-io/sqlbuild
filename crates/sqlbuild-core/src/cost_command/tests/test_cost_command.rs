use crate::cost_command::main::_cost_history_limit::cost_history_limit;
use crate::cost_command::main::accepts_cost_refresh::accepts_cost_refresh;
use crate::cost_command::main::is_zero_cost_duration::is_zero_cost_duration;
use crate::cost_command::main::select_cost_run::select_cost_run;
use crate::cost_command::main::validate_cost_request::validate_cost_request;
use crate::cost_command::models::CostCoverage;
use crate::cost_command::tests::test_types::{
    RefreshTestCase, ScalarTestCase, SelectionTestCase, ValidationTestCase,
};

#[test]
fn given_request_when_validating_then_refuses_with_the_expected_code() {
    let test_cases = [
        ValidationTestCase {
            description: "a negative limit",
            selector: "history",
            limits: (Some(-1), false),
            sort_field: "completed",
            has_bounds: false,
            expected_code: Some("C709"),
        },
        ValidationTestCase {
            description: "a limit with no limit",
            selector: "latest",
            limits: (Some(1), true),
            sort_field: "cost",
            has_bounds: false,
            expected_code: Some("C710"),
        },
        ValidationTestCase {
            description: "a breakdown field in history",
            selector: "history",
            limits: (None, false),
            sort_field: "busy",
            has_bounds: true,
            expected_code: Some("C701"),
        },
        ValidationTestCase {
            description: "bounds on a run breakdown",
            selector: "latest",
            limits: (None, false),
            sort_field: "nope",
            has_bounds: true,
            expected_code: Some("C702"),
        },
        ValidationTestCase {
            description: "an unknown breakdown field",
            selector: "run-1",
            limits: (None, false),
            sort_field: "completed",
            has_bounds: false,
            expected_code: Some("C703"),
        },
        ValidationTestCase {
            description: "a valid breakdown",
            selector: "run-1",
            limits: (Some(0), false),
            sort_field: "warehouse",
            has_bounds: false,
            expected_code: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            validate_cost_request(
                test_case.selector,
                test_case.limits,
                test_case.sort_field,
                test_case.has_bounds
            )
            .err()
            .map(|error| error.code),
            test_case.expected_code,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selector_when_selecting_run_then_prefers_exact_then_unique_prefix() {
    let run_ids: Vec<String> = ["run-a1", "run-a2", "run-b1", "run-a"]
        .iter()
        .map(|run_id| (*run_id).to_owned())
        .collect();
    let test_cases = [
        SelectionTestCase {
            description: "an exact id wins over prefixes",
            selector: "run-a",
            expected_index: Some(3),
            expected_code: None,
        },
        SelectionTestCase {
            description: "a unique prefix",
            selector: "run-b",
            expected_index: Some(2),
            expected_code: None,
        },
        SelectionTestCase {
            description: "no match",
            selector: "nope",
            expected_index: None,
            expected_code: Some("C705"),
        },
        SelectionTestCase {
            description: "a shared prefix",
            selector: "run",
            expected_index: None,
            expected_code: Some("C706"),
        },
    ];

    for test_case in test_cases {
        let selected = select_cost_run(&run_ids, test_case.selector);
        assert_eq!(
            selected.clone().ok(),
            test_case.expected_index,
            "{}",
            test_case.description
        );
        assert_eq!(
            selected.err().map(|error| error.code),
            test_case.expected_code,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_refreshed_summary_when_comparing_then_never_regresses() {
    let coverage = |status: &str, counts: (i64, i64, i64), limitations: &[&str]| CostCoverage {
        status: status.to_owned(),
        expected_statement_count: counts.0,
        observed_statement_count: counts.1,
        query_count: counts.2,
        limitations: limitations.iter().map(|text| (*text).to_owned()).collect(),
    };
    let test_cases = [
        RefreshTestCase {
            description: "a complete refresh of a pending run",
            current: coverage("pending", (1, 0, 0), &[]),
            candidate: coverage("complete", (1, 1, 1), &[]),
            expected_accepted: Some(true),
        },
        RefreshTestCase {
            description: "a failure never replaces a partial run",
            current: coverage("partial", (17, 17, 2), &[]),
            candidate: coverage("collection_failed", (17, 0, 0), &[]),
            expected_accepted: Some(false),
        },
        RefreshTestCase {
            description: "a ledger failure needs a coverage gain",
            current: coverage(
                "partial",
                (2, 2, 2),
                &["Statement-ledger persistence failed (x)"],
            ),
            candidate: coverage("complete", (2, 2, 2), &[]),
            expected_accepted: Some(false),
        },
        RefreshTestCase {
            description: "an unknown status",
            current: coverage("partial", (0, 0, 0), &[]),
            candidate: coverage("other", (0, 0, 0), &[]),
            expected_accepted: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            accepts_cost_refresh(&test_case.current, &test_case.candidate),
            test_case.expected_accepted,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_values_when_checking_zero_durations_and_limits_then_matches_cost_rules() {
    let test_cases = [
        ScalarTestCase {
            description: "zero days with the default limit",
            value: "00d",
            limits: (None, false),
            expected_zero: true,
            expected_limit: Some(10),
        },
        ScalarTestCase {
            description: "seconds are not a zero window and no limit is unbounded",
            value: "0s",
            limits: (None, true),
            expected_zero: false,
            expected_limit: None,
        },
        ScalarTestCase {
            description: "one day with an explicit limit",
            value: "1d",
            limits: (Some(3), false),
            expected_zero: false,
            expected_limit: Some(3),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            is_zero_cost_duration(test_case.value),
            test_case.expected_zero,
            "{}",
            test_case.description
        );
        assert_eq!(
            cost_history_limit(test_case.limits.0, test_case.limits.1),
            test_case.expected_limit,
            "{}",
            test_case.description
        );
    }
}

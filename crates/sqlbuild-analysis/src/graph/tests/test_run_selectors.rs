use crate::graph::main::run_selector_ids::run_selector_ids;
use crate::graph::tests::helpers::{failure, names, run_selection};
use crate::graph::tests::test_types::{
    RunSelectorErrorTestCase, RunSelectorIdsTestCase, RunSelectorTestCase,
};

#[test]
fn given_a_past_run_and_audits_when_resolving_then_run_and_audit_selectors_compose() {
    let test_cases = [
        RunSelectorTestCase {
            description: "failed: selects the run's failed nodes and failed audits, and the functions they need",
            select: &["failed:01a1d3b9"],
            exclude: &[],
            expected_keys: &["audit:orders_match_payments", "model:orders", "udf:tax"],
        },
        RunSelectorTestCase {
            description: "failed:+ adds what reads the failed nodes; an audit has nothing downstream",
            select: &["failed:01a1d3b9+"],
            exclude: &[],
            expected_keys: &[
                "audit:orders_match_payments",
                "model:customer_orders",
                "model:order_items",
                "model:orders",
                "udf:tax",
            ],
        },
        RunSelectorTestCase {
            description: "run: selects what the run selected that the project still has",
            select: &["run:01a1d3b9"],
            exclude: &[],
            expected_keys: &[
                "model:customers",
                "model:order_items",
                "model:orders",
                "udf:tax",
            ],
        },
        RunSelectorTestCase {
            description: "skipped: selects what the run skipped, and the functions it needs",
            select: &["skipped:01a1d3b9"],
            exclude: &[],
            expected_keys: &["model:order_items", "udf:tax"],
        },
        RunSelectorTestCase {
            description: "an intersection keeps the failed nodes with the tag; audits have no tags",
            select: &["failed:01a1d3b9+,tag:finance"],
            exclude: &[],
            expected_keys: &["model:order_items", "udf:tax"],
        },
        RunSelectorTestCase {
            description: "an exclusion drops the run's audits",
            select: &["failed:01a1d3b9"],
            exclude: &["audit:*"],
            expected_keys: &["model:orders", "udf:tax"],
        },
        RunSelectorTestCase {
            description: "a run selector joins other selectors",
            select: &["customers failed:01a1d3b9"],
            exclude: &[],
            expected_keys: &[
                "audit:orders_match_payments",
                "model:customers",
                "model:orders",
                "udf:tax",
            ],
        },
        RunSelectorTestCase {
            description: "audit: selects an audit by name or pattern",
            select: &["audit:not_null_*"],
            exclude: &[],
            expected_keys: &["audit:not_null_orders_id"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            run_selection(true, test_case.select, test_case.exclude),
            Ok(names(test_case.expected_keys)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_run_and_audit_selectors_when_unresolvable_then_fails_with_message() {
    let test_cases = [
        RunSelectorErrorTestCase {
            description: "an unknown audit suggests the closest",
            attached: true,
            select: &["audit:orders_match_payment"],
            expected_error: (
                "S007",
                "unknown audit name 'orders_match_payment'",
                Some("did you mean 'orders_match_payments'?"),
            ),
        },
        RunSelectorErrorTestCase {
            description: "a command that selects no audits refuses audit:",
            attached: false,
            select: &["audit:orders_match_payments"],
            expected_error: (
                "S010",
                "selector 'audit:orders_match_payments' selects an audit; only plan, build, test, audit and check select audits",
                None,
            ),
        },
        RunSelectorErrorTestCase {
            description: "a command that reads no past runs refuses failed:",
            attached: false,
            select: &["failed:01a1d3b9+"],
            expected_error: (
                "S010",
                "selector 'failed:01a1d3b9' selects from a past run; only plan, build, test, audit and check read past runs",
                None,
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            run_selection(test_case.attached, test_case.select, &[]),
            Err(failure(test_case.expected_error)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selectors_when_scanning_then_run_ids_are_listed_as_typed() {
    let test_cases = [
        RunSelectorIdsTestCase {
            description: "each run kind names its run, inside intersections and with expansion",
            selectors: &[
                "failed:01a1d3b9+ customers,run:01a1d3c0",
                "skipped:01a1d3b9",
            ],
            expected_ids: &["01a1d3b9", "01a1d3c0"],
        },
        RunSelectorIdsTestCase {
            description: "other selectors name no run",
            selectors: &["job:nightly audit:not_null_orders_id tag:daily"],
            expected_ids: &[],
        },
    ];
    for test_case in test_cases {
        let selectors: Vec<String> = test_case
            .selectors
            .iter()
            .map(|selector| (*selector).to_owned())
            .collect();
        assert_eq!(
            run_selector_ids(&selectors),
            names(test_case.expected_ids),
            "{}",
            test_case.description
        );
    }
}

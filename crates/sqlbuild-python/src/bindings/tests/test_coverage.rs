use std::collections::BTreeSet;

use crate::bindings::_helpers::compile_delta::driver::names_uncompiled_model;
use crate::bindings::tests::helpers::{coverage, names, order, strings};
use crate::bindings::tests::test_types::{CoverageTestCase, UncompiledModelTestCase};
use sqlbuild_python_base::compile_context::main::coverage::coverage_reasons::coverage_reasons;
use sqlbuild_python_base::compile_context::main::coverage::merged_coverage::merged_coverage;
use sqlbuild_python_base::compile_context::main::coverage::merged_order::merged_order;
use sqlbuild_python_base::compile_context::models::ModelCoverage;

const DISABLED: &str = "SQL analysis disabled";
const OPEN_INPUT: &str = "open input: raw_orders";

#[test]
fn given_stored_and_fresh_coverage_when_merging_then_reasons_match_a_whole_project_compile() {
    let chain: &[(&str, &[&str])] = &[
        ("orders", &[]),
        ("payments", &["orders"]),
        ("customers", &["payments"]),
        ("returns", &[]),
    ];
    let test_cases = [
        CoverageTestCase {
            description: "a stored model without SQL analysis keeps its reason",
            stored: vec![coverage("returns", (true, false), &[])],
            fresh: vec![coverage("orders", (false, false), &[])],
            stored_order: order(chain),
            fresh_order: order(&[("orders", &[])]),
            expected_reasons: order(&[("returns", &[DISABLED])]),
        },
        CoverageTestCase {
            description: "a rerun model turning invalid makes every stored dependent partial",
            stored: vec![coverage("customers", (false, false), &[OPEN_INPUT])],
            fresh: vec![coverage("orders", (false, true), &[])],
            stored_order: order(chain),
            fresh_order: order(&[("orders", &[])]),
            expected_reasons: order(&[
                ("payments", &["depends on invalid model orders"]),
                (
                    "customers",
                    &["depends on invalid model orders", OPEN_INPUT],
                ),
            ]),
        },
        CoverageTestCase {
            description: "a rerun model fixed drops the reasons its dependents had from it",
            stored: vec![coverage("orders", (false, true), &[])],
            fresh: vec![coverage("orders", (false, false), &[])],
            stored_order: order(chain),
            fresh_order: order(&[("orders", &[])]),
            expected_reasons: vec![],
        },
        CoverageTestCase {
            description: "a rerun model reading a stored invalid model becomes partial",
            stored: vec![coverage("returns", (false, true), &[])],
            fresh: vec![coverage("orders", (false, false), &[])],
            stored_order: order(chain),
            fresh_order: order(&[("orders", &["returns"])]),
            expected_reasons: order(&[
                ("orders", &["depends on invalid model returns"]),
                ("payments", &["depends on invalid model returns"]),
                ("customers", &["depends on invalid model returns"]),
            ]),
        },
    ];

    for test_case in test_cases {
        let facts: Vec<ModelCoverage> = merged_coverage(&test_case.stored, &test_case.fresh);
        let project_order: Vec<(String, Vec<String>)> =
            merged_order(&test_case.stored_order, &test_case.fresh_order);

        assert_eq!(
            coverage_reasons(&project_order, &facts),
            test_case.expected_reasons,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_missing_model_errors_when_judging_a_delta_then_only_uncompiled_models_rule_it_out() {
    let test_cases = [
        UncompiledModelTestCase {
            description: "an expectation of a project model the delta did not compile",
            message: "test 'orders_check' expects model 'payments' which does not exist",
            expected_uncompiled: true,
        },
        UncompiledModelTestCase {
            description: "a reference to a model the project lacks is a real error",
            message: "references unknown model 'refunds'",
            expected_uncompiled: false,
        },
        UncompiledModelTestCase {
            description: "a reference to a model the delta compiled is a real error",
            message: "references unknown model 'orders'",
            expected_uncompiled: false,
        },
        UncompiledModelTestCase {
            description: "an unrelated error naming a project model",
            message: "model 'payments' has a type error",
            expected_uncompiled: false,
        },
    ];
    let model_names: Vec<String> = strings(&["orders", "payments"]);
    let rerun: BTreeSet<String> = names(&["orders"]);

    for test_case in test_cases {
        assert_eq!(
            names_uncompiled_model(test_case.message, &model_names, &rerun),
            test_case.expected_uncompiled,
            "{}",
            test_case.description
        );
    }
}

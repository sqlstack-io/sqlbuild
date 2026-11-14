use fensu_native::rules::constants as fensu;

use crate::python_rules::tests::helpers::{
    core_rule_faults, distinct_python_codes, is_selected, project_wide_python_rules,
};
use crate::python_rules::tests::test_types::{
    CatalogueTestCase, CoreRuleTestCase, SelectionTestCase,
};
use crate::rules::_helpers::catalogue::catalogue;

#[test]
fn given_the_catalogue_when_listing_python_rules_then_each_has_its_own_project_wide_code() {
    let test_cases = [
        CatalogueTestCase {
            description: "two SQLBuild layout rules and 62 Fensu core rules have distinct codes",
            count: distinct_python_codes,
            expected_count: 64,
        },
        CatalogueTestCase {
            description: "every Python rule reports against the project in the python family",
            count: project_wide_python_rules,
            expected_count: 64,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            (test_case.count)(&catalogue()),
            test_case.expected_count,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_python_selectors_when_selecting_then_opt_in_rules_need_their_exact_code() {
    let test_cases = [
        SelectionTestCase {
            description: "the family selects a default rule",
            select: vec!["SQBRPYTHON".to_owned()],
            code: "SQBRPYTHON101",
            expected_selected: true,
        },
        SelectionTestCase {
            description: "the family leaves out opt-in unreachable definitions",
            select: vec!["SQBRPYTHON".to_owned()],
            code: "SQBRPYTHON306",
            expected_selected: false,
        },
        SelectionTestCase {
            description: "the family leaves out opt-in parameter mutation",
            select: vec!["SQBRPYTHON".to_owned()],
            code: "SQBRPYTHON507",
            expected_selected: false,
        },
        SelectionTestCase {
            description: "the exact code selects an opt-in rule",
            select: vec!["SQBRPYTHON306".to_owned()],
            code: "SQBRPYTHON306",
            expected_selected: true,
        },
        SelectionTestCase {
            description: "the whole built-in namespace leaves Python rules out",
            select: vec!["SQBR".to_owned()],
            code: "SQBRPYTHON101",
            expected_selected: false,
        },
        SelectionTestCase {
            description: "the built-in namespace with the Python family selects both",
            select: vec!["SQBR".to_owned(), "SQBRPYTHON".to_owned()],
            code: "SQBRPYTHON101",
            expected_selected: true,
        },
        SelectionTestCase {
            description: "the default selection leaves Python rules out",
            select: Vec::new(),
            code: "SQBRPYTHON101",
            expected_selected: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            is_selected(&test_case.select, test_case.code),
            test_case.expected_selected,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_python_source_when_evaluating_a_fensu_core_rule_then_reports_its_faults() {
    let test_cases = [
        CoreRuleTestCase {
            description: "an unannotated parameter is reported on its line",
            source: "def total(amount) -> int:\n    return amount\n",
            fensu_code: fensu::PARAMETER_ANNOTATION_CODE,
            expected_faults: vec![(fensu::PARAMETER_ANNOTATION_CODE.to_owned(), 1)],
        },
        CoreRuleTestCase {
            description: "an annotated parameter passes",
            source: "def total(amount: int) -> int:\n    return amount\n",
            fensu_code: fensu::PARAMETER_ANNOTATION_CODE,
            expected_faults: Vec::new(),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            core_rule_faults(test_case.source, test_case.fensu_code),
            test_case.expected_faults,
            "{}",
            test_case.description
        );
    }
}

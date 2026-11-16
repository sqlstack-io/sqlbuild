use crate::tests::helpers::install_test_identity;
use fensu_native::rules::constants as fensu;

use crate::python_rules::tests::helpers::{
    core_rule_faults, distinct_python_codes, is_selected, project_wide_python_rules, source_keys,
};
use crate::python_rules::tests::test_types::{
    CatalogueTestCase, CoreRuleTestCase, OptionsTestCase, SelectionTestCase, SourceKeyTestCase,
};
use crate::rules::_helpers::dispatch::catalogue::catalogue;
use crate::rules::main::builtin_options::builtin_options;
use crate::rules::models::BuiltinOptionKind;

#[test]
fn given_the_catalogue_when_listing_python_rules_then_each_has_its_own_project_wide_code() {
    install_test_identity();
    let test_cases = [
        CatalogueTestCase {
            description: "21 SQLBuild layout, unit, provider, orchestration and test placement rules and 86 Fensu core and test rules have distinct codes",
            count: distinct_python_codes,
            expected_count: 108,
        },
        CatalogueTestCase {
            description: "every Python rule reports against the project in the python family",
            count: project_wide_python_rules,
            expected_count: 108,
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
    install_test_identity();
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
            description: "the built-in namespace with one Python code leaves the other Python rules out",
            select: vec!["SQBR".to_owned(), "SQBRPYTHON101".to_owned()],
            code: "SQBRPYTHON613",
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

#[test]
fn given_python_rules_when_reading_their_options_then_fensu_thresholds_are_whole_numbers() {
    let test_cases = [
        OptionsTestCase {
            description: "a shape rule declares its one threshold with Fensu's default",
            code: "SQBRPYTHON501",
            expected_options: vec![(
                "max_statements",
                BuiltinOptionKind::Integer {
                    default: 40,
                    minimum: 0,
                },
            )],
        },
        OptionsTestCase {
            description: "a package layout rule declares its module count and role depth",
            code: "SQBRPYTHON610",
            expected_options: vec![
                (
                    "max_helpers_container_modules",
                    BuiltinOptionKind::Integer {
                        default: 10,
                        minimum: 0,
                    },
                ),
                (
                    "max_role_depth",
                    BuiltinOptionKind::Integer {
                        default: 1,
                        minimum: 0,
                    },
                ),
            ],
        },
        OptionsTestCase {
            description: "a rule without thresholds declares no options",
            code: "SQBRPYTHON101",
            expected_options: Vec::new(),
        },
    ];
    for test_case in test_cases {
        let declared: Vec<(&str, BuiltinOptionKind)> = builtin_options(test_case.code)
            .into_iter()
            .map(|option| (option.name, option.kind))
            .collect();
        assert_eq!(
            declared, test_case.expected_options,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_python_edit_when_preparing_rules_then_only_python_selections_see_it() {
    install_test_identity();
    const ORIGINAL: &str = "def total(amount: int) -> int:\n    return amount\n";
    let test_cases = [
        SourceKeyTestCase {
            description: "an edit changes the key when the Python family is selected",
            select: &["SQBRPYTHON"],
            edited_source: "def total(amount):\n    return amount\n",
            expected_same_key: false,
        },
        SourceKeyTestCase {
            description: "unchanged Python keeps the key",
            select: &["SQBRPYTHON"],
            edited_source: ORIGINAL,
            expected_same_key: true,
        },
        SourceKeyTestCase {
            description: "an edit is not read when only SQL rules run",
            select: &["SQBR"],
            edited_source: "def total(amount):\n    return amount\n",
            expected_same_key: true,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            {
                let (before, after) =
                    source_keys(test_case.select, ORIGINAL, test_case.edited_source);
                before == after
            },
            test_case.expected_same_key,
            "{}",
            test_case.description
        );
    }
}

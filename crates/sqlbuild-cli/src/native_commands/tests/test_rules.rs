use crate::launcher::main::install_native_build_identity::install_native_build_identity;
use crate::native_commands::_helpers::command::rules::run_rules;
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{RulesAction, RulesOptions};
use crate::native_commands::tests::helpers::write_tree;
use crate::native_commands::tests::test_types::{RulesTestCase, output_start};

const PROJECT: &str = "name = \"shop\"\nadapter = \"duckdb\"\n";
const OPTIONS_PROJECT: &str = "name = \"shop\"\nadapter = \"duckdb\"\n\n[rules.rule_options.SQBRCONTRACT102]\nallow_numeric_indicators = true\n";

#[test]
fn given_project_when_inspecting_rules_then_builtin_catalogue_prints_unless_python_owns_it() {
    install_native_build_identity().expect("the build identity installs once");
    let test_cases = [
        RulesTestCase {
            description: "list prints code and message lines",
            directories: &[],
            project_file: PROJECT,
            action: RulesAction::List,
            selector: None,
            json: false,
            expected_output: Ok((
                "SQBRCONTRACT101  models must declare an enforced output contract\n",
                0,
            )),
        },
        RulesTestCase {
            description: "list --json prints the sorted document",
            directories: &[],
            project_file: PROJECT,
            action: RulesAction::List,
            selector: None,
            json: true,
            expected_output: Ok((
                "{\"rules\": [{\"code\": \"SQBRCONTRACT101\", \"custom\": false, \"family\": \"SQBRCONTRACT\", ",
                0,
            )),
        },
        RulesTestCase {
            description: "show prints the rule",
            directories: &[],
            project_file: PROJECT,
            action: RulesAction::Show,
            selector: Some("SQBRMODEL101"),
            json: false,
            expected_output: Ok((
                "SQBRMODEL101: dependency-import-ctes\nFamily: SQBRMODEL\n",
                0,
            )),
        },
        RulesTestCase {
            description: "an unknown rule is a usage failure",
            directories: &[],
            project_file: PROJECT,
            action: RulesAction::Show,
            selector: Some("SQBRNOPE101"),
            json: false,
            expected_output: Ok(("Unknown rule: SQBRNOPE101\n", 2)),
        },
        RulesTestCase {
            description: "custom rules are imported by Python",
            directories: &["rules"],
            project_file: PROJECT,
            action: RulesAction::List,
            selector: None,
            json: false,
            expected_output: Err(PythonNeeded::Project),
        },
        RulesTestCase {
            description: "configured options are rendered by Python",
            directories: &[],
            project_file: OPTIONS_PROJECT,
            action: RulesAction::List,
            selector: None,
            json: false,
            expected_output: Err(PythonNeeded::Project),
        },
    ];

    for test_case in test_cases {
        let root = tempfile::tempdir().expect("temporary directory");
        write_tree(
            root.path(),
            test_case.directories,
            &[("sqlbuild_project.toml", test_case.project_file)],
        );

        let output = run_rules(
            root.path(),
            &RulesOptions {
                action: test_case.action,
                selector: test_case.selector.map(str::to_owned),
                json: test_case.json,
            },
        );

        let length: usize = test_case
            .expected_output
            .map_or(0, |(prefix, _)| prefix.chars().count());

        assert_eq!(
            output.map(|output| output_start(&output, length)),
            test_case
                .expected_output
                .map(|(prefix, code)| (prefix.to_owned(), code)),
            "{}",
            test_case.description
        );
    }
}

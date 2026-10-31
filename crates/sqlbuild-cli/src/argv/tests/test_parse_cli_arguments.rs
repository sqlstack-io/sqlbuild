use std::collections::BTreeMap;

use serde_json::json;

use crate::argv::main::job_settings_arguments::job_settings_arguments;
use crate::argv::models::{ArgvOutcome, Value};
use crate::argv::tests::helpers::{parse, strings};
use crate::argv::tests::test_types::{
    ExitTestCase, HelpTextTestCase, JobSettingsRejectionTestCase, JobSettingsTestCase,
    ParsedValueTestCase,
};

#[test]
fn given_accepted_arguments_when_parsing_then_attributes_hold_the_parsed_values() {
    let test_cases = [
        ParsedValueTestCase {
            description: "positional, trailing and --select selectors merge",
            argv: &["build", "orders", "--select", "customers", "products"],
            attribute: "select",
            expected_value: strings(&["orders", "customers", "products"]),
        },
        ParsedValueTestCase {
            description: "an unset concurrency stays None",
            argv: &["build"],
            attribute: "concurrency",
            expected_value: Value::None,
        },
        ParsedValueTestCase {
            description: "an abbreviated option with = keeps its value",
            argv: &["build", "--conc=4"],
            attribute: "concurrency",
            expected_value: Value::Int("4".to_owned()),
        },
        ParsedValueTestCase {
            description: "dbt passthrough keeps arguments after the separator",
            argv: &["dbt", "plan", "--no-color", "--", "--no-color"],
            attribute: "dbt_args",
            expected_value: strings(&["--", "--no-color"]),
        },
        ParsedValueTestCase {
            description: "clone still accepts the hidden --no-sql-analysis flag",
            argv: &["clone", "--from", "prod", "--no-sql-analysis"],
            attribute: "no_sql_validation",
            expected_value: Value::Bool(true),
        },
        ParsedValueTestCase {
            description: "a nested sub-command's dest is set",
            argv: &["contract", "diff", "--from", "prod"],
            attribute: "contract_command",
            expected_value: Value::Str("diff".to_owned()),
        },
    ];

    for test_case in test_cases {
        let outcome: ArgvOutcome = parse(test_case.argv);
        let parsed = outcome.parsed.expect(test_case.description);
        assert_eq!(
            parsed.get(test_case.attribute),
            Some(&test_case.expected_value),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_help_version_or_rejected_arguments_when_parsing_then_exits_after_writing() {
    let test_cases = [
        ExitTestCase {
            description: "version prints the installed version",
            argv: &["--version"],
            expected_exit: 0,
            expected_last_write: "sqb 1.2.3\n",
        },
        ExitTestCase {
            description: "an invalid integer is a coded parser error",
            argv: &["build", "--concurrency", "x1"],
            expected_exit: 2,
            expected_last_write: "sqb build: error[C900]: argument --concurrency: invalid int value: 'x1'\n\n",
        },
        ExitTestCase {
            description: "an ambiguous abbreviation lists its matches",
            argv: &["build", "--allow-s"],
            expected_exit: 2,
            expected_last_write: "sqb build: error[C900]: ambiguous option: --allow-s could match --allow-snapshot-full-refresh, --allow-snapshot-schema-change\n\n",
        },
        ExitTestCase {
            description: "a post-parse rule reports through the root parser",
            argv: &["build", "--concurrency", "0"],
            expected_exit: 2,
            expected_last_write: "sqb: error[C900]: --concurrency must be >= 1\n\n",
        },
        ExitTestCase {
            description: "a missing required sub-command is named",
            argv: &["rules"],
            expected_exit: 2,
            expected_last_write: "sqb rules: error[C900]: the following arguments are required: rules_command\n\n",
        },
        ExitTestCase {
            description: "a rename target without a new name stays a usage error",
            argv: &["rename", "stg_orders"],
            expected_exit: 2,
            expected_last_write: "sqb rename: error[C900]: the following arguments are required: NEW_NAME\n\n",
        },
        ExitTestCase {
            description: "an invalid review choice exits as a review setup failure",
            argv: &["review", "--downstream", "some"],
            expected_exit: 3,
            expected_last_write: "sqb review: error[C900]: argument --downstream: invalid choice: 'some' (choose from 'critical', 'all', 'none')\n\n",
        },
        ExitTestCase {
            description: "an unknown review option exits as a review setup failure",
            argv: &["review", "--no-such-option"],
            expected_exit: 3,
            expected_last_write: "sqb: error[C900]: unrecognized arguments: --no-such-option\n\n",
        },
        ExitTestCase {
            description: "a rename without a target or a file stays a usage error",
            argv: &["rename"],
            expected_exit: 2,
            expected_last_write: "sqb rename: error[C900]: the following arguments are required: TARGET, NEW_NAME\n\n",
        },
    ];

    for test_case in test_cases {
        let outcome: ArgvOutcome = parse(test_case.argv);
        assert_eq!(
            outcome.parsed,
            Err(test_case.expected_exit),
            "{}",
            test_case.description
        );
        assert_eq!(
            outcome.writes.last().map(|(_, text)| text.as_str()),
            Some(test_case.expected_last_write),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_command_help_when_parsing_then_hidden_options_are_left_out() {
    let test_cases = [
        HelpTextTestCase {
            description: "clone help omits the no-op SQL analysis flag",
            argv: &["clone", "--help"],
            expected_present: &["--from FROM_TARGET", "--select"],
            expected_absent: &["--no-sql-analysis", "--no-sql-validation"],
        },
        HelpTextTestCase {
            description: "build help keeps the SQL analysis flag",
            argv: &["build", "--help"],
            expected_present: &["--no-sql-analysis"],
            expected_absent: &[],
        },
    ];

    for test_case in test_cases {
        let outcome: ArgvOutcome = parse(test_case.argv);
        let help: String = outcome
            .writes
            .iter()
            .map(|(_, text)| text.as_str())
            .collect();
        let present: Vec<&str> = test_case
            .expected_present
            .iter()
            .copied()
            .filter(|fragment| help.contains(fragment))
            .collect();
        let absent: Vec<&str> = test_case
            .expected_absent
            .iter()
            .copied()
            .filter(|fragment| !help.contains(fragment))
            .collect();
        assert_eq!(outcome.parsed, Err(0), "{}", test_case.description);
        assert_eq!(present, test_case.expected_present, "{help}");
        assert_eq!(absent, test_case.expected_absent, "{help}");
    }
}

#[test]
fn given_job_settings_when_validating_then_values_survive_the_build_parser() {
    let test_cases = [
        JobSettingsTestCase {
            description: "inverse flags and option-like values survive the build parser",
            settings: BTreeMap::from([
                ("concurrency".into(), json!(3)),
                ("load_sources".into(), json!(false)),
                ("reload".into(), json!(false)),
                ("run_tests".into(), json!(false)),
                ("full_refresh".into(), json!(true)),
                ("warehouse".into(), json!("--full-refresh")),
            ]),
            expected_values: vec![
                ("concurrency", Value::Int("3".into())),
                ("load_sources", Value::Bool(false)),
                ("reload", Value::Bool(false)),
                ("run_tests", Value::Bool(false)),
                ("full_refresh", Value::Bool(true)),
                ("warehouse", Value::Str("--full-refresh".into())),
            ],
            expected_empty: false,
        },
        JobSettingsTestCase {
            description: "default-valued flags do not become explicit options",
            settings: BTreeMap::from([
                ("run_tests".into(), json!(true)),
                ("full_refresh".into(), json!(false)),
                ("debug".into(), json!(false)),
            ]),
            expected_values: vec![],
            expected_empty: true,
        },
    ];
    for test_case in test_cases {
        let settings = job_settings_arguments(&test_case.settings).expect(test_case.description);
        assert_eq!(
            settings.is_empty(),
            test_case.expected_empty,
            "{}",
            test_case.description
        );
        let mut arguments: Vec<String> = vec!["build".to_owned()];
        arguments.extend(settings);
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        let parsed = parse(&arguments).parsed.expect(test_case.description);
        for (name, expected_value) in test_case.expected_values {
            assert_eq!(
                parsed.get(name),
                Some(&expected_value),
                "{}",
                test_case.description
            );
        }
    }
}

#[test]
fn given_conflicting_job_settings_when_validating_then_options_are_rejected() {
    let test_cases = [
        JobSettingsRejectionTestCase {
            description: "no-load conflicts with reload",
            settings: BTreeMap::from([
                ("load_sources".into(), json!(false)),
                ("reload".into(), json!(true)),
            ]),
            expected_error: "mutually exclusive",
        },
        JobSettingsRejectionTestCase {
            description: "load conflicts with reload",
            settings: BTreeMap::from([
                ("load_sources".into(), json!(true)),
                ("reload".into(), json!(true)),
            ]),
            expected_error: "mutually exclusive",
        },
    ];
    for test_case in test_cases {
        assert!(
            job_settings_arguments(&test_case.settings)
                .expect_err(test_case.description)
                .contains(test_case.expected_error),
            "{}",
            test_case.description
        );
    }
}

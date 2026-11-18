use crate::argv::models::Namespace;
use crate::native_commands::_helpers::command::decision::native_invocation;
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{NativeCommand, RulesAction, RulesOptions, SkillsOptions};
use crate::native_commands::tests::helpers::parse;
use crate::native_commands::tests::test_types::{DecisionTestCase, ParseTestCase};

#[test]
fn given_parsed_command_when_deciding_then_only_python_free_commands_run_natively() {
    let test_cases = [
        DecisionTestCase {
            description: "a bare sqb has no handler",
            argv: &[],
            expected_command: Ok(NativeCommand::Nothing),
            expected_creates_project: false,
        },
        DecisionTestCase {
            description: "clean has no handler",
            argv: &["clean"],
            expected_command: Ok(NativeCommand::Nothing),
            expected_creates_project: false,
        },
        DecisionTestCase {
            description: "init creates a project",
            argv: &["init", "--project-dir", "shop"],
            expected_command: Ok(NativeCommand::Init),
            expected_creates_project: true,
        },
        DecisionTestCase {
            description: "skills with targets",
            argv: &[
                "skills", "--target", "claude", "--target", "agents", "--force",
            ],
            expected_command: Ok(NativeCommand::Skills(SkillsOptions {
                global_install: false,
                targets: vec!["claude".to_owned(), "agents".to_owned()],
                force: true,
            })),
            expected_creates_project: false,
        },
        DecisionTestCase {
            description: "rules list as JSON",
            argv: &["rules", "--json", "list"],
            expected_command: Ok(NativeCommand::Rules(RulesOptions {
                action: RulesAction::List,
                selector: None,
                json: true,
            })),
            expected_creates_project: false,
        },
        DecisionTestCase {
            description: "rules show",
            argv: &["rules", "show", "SQBRMODEL101"],
            expected_command: Ok(NativeCommand::Rules(RulesOptions {
                action: RulesAction::Show,
                selector: Some("SQBRMODEL101".to_owned()),
                json: false,
            })),
            expected_creates_project: false,
        },
        DecisionTestCase {
            description: "rules run evaluates the project in Python",
            argv: &["rules", "run", "SQBRMODEL"],
            expected_command: Err(PythonNeeded::Command),
            expected_creates_project: false,
        },
        DecisionTestCase {
            description: "debug output is Python's",
            argv: &["clean", "--debug"],
            expected_command: Err(PythonNeeded::Command),
            expected_creates_project: false,
        },
        DecisionTestCase {
            description: "compile needs Python",
            argv: &["compile"],
            expected_command: Err(PythonNeeded::Command),
            expected_creates_project: false,
        },
    ];

    for test_case in test_cases {
        let (outcome, _) = parse(test_case.argv);
        let namespace: Namespace = outcome.parsed.expect(test_case.description);

        let decided = native_invocation(&namespace);

        assert_eq!(
            decided
                .as_ref()
                .map(|invocation| invocation.command.clone())
                .map_err(|reason| *reason),
            test_case.expected_command.clone(),
            "{}",
            test_case.description
        );
        assert_eq!(
            decided.is_ok_and(|invocation| invocation.creates_project),
            test_case.expected_creates_project,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_command_line_when_parsing_natively_then_only_python_converters_need_python() {
    let test_cases = [
        ParseTestCase {
            description: "--vars JSON is parsed by Python",
            argv: &["compile", "--vars", "{}"],
            expected_needs_python: true,
        },
        ParseTestCase {
            description: "an integer option converts natively",
            argv: &["build", "--concurrency", "4"],
            expected_needs_python: false,
        },
    ];

    for test_case in test_cases {
        let (_, host) = parse(test_case.argv);

        assert_eq!(
            host.needs_python, test_case.expected_needs_python,
            "{}",
            test_case.description
        );
    }
}

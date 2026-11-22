use crate::argv::main::command_project_and_target::command_project_and_target;
use crate::argv::tests::test_types::CommandProjectTestCase;

#[test]
fn given_a_command_line_when_reading_its_project_and_target_then_typed_options_still_parse() {
    let test_cases = [
        CommandProjectTestCase {
            description: "no typed options",
            argv: &["--project-dir", "shop", "build", "--target", "dev"],
            expected: Some(("shop", Some("dev"))),
        },
        CommandProjectTestCase {
            description: "a concurrency",
            argv: &[
                "--project-dir",
                "shop",
                "build",
                "--concurrency",
                "4",
                "--target",
                "dev",
            ],
            expected: Some(("shop", Some("dev"))),
        },
        CommandProjectTestCase {
            description: "run variables",
            argv: &[
                "--project-dir",
                "shop",
                "build",
                "--vars",
                "region: north",
                "--target",
                "dev",
            ],
            expected: Some(("shop", Some("dev"))),
        },
        CommandProjectTestCase {
            description: "a command whose numeric defaults are validated",
            argv: &[
                "--project-dir",
                "shop",
                "scope",
                "--at",
                "models",
                "--browse",
                "models",
            ],
            expected: Some(("shop", None)),
        },
        CommandProjectTestCase {
            description: "an unknown option",
            argv: &["--project-dir", "shop", "build", "--no-such-option"],
            expected: None,
        },
    ];

    for test_case in test_cases {
        let arguments: Vec<String> = test_case.argv.iter().copied().map(str::to_owned).collect();
        assert_eq!(
            command_project_and_target(&arguments),
            test_case
                .expected
                .map(|(project, target)| (Some(project.to_owned()), target.map(str::to_owned))),
            "{}",
            test_case.description
        );
    }
}

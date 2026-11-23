use crate::cli_text::models::CliStyle;
use crate::init_command::main::init_project_name::init_project_name;
use crate::init_command::main::init_project_toml::init_project_toml;
use crate::init_command::main::render_init_summary::render_init_summary;
use crate::init_command::tests::test_types::{NameTestCase, SummaryTestCase};

#[test]
fn given_directory_name_when_naming_project_then_normalizes_separators_and_case() {
    let test_cases = [
        NameTestCase {
            description: "dashes and spaces become underscores",
            directory_name: "Demo-Project Name",
            expected_name: "demo_project_name",
        },
        NameTestCase {
            description: "non-ASCII letters lowercase",
            directory_name: "UPPER_Ünïcode",
            expected_name: "upper_ünïcode",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            init_project_name(test_case.directory_name),
            test_case.expected_name,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_project_name_when_scaffolding_then_writes_config_and_summary() {
    let test_cases = [SummaryTestCase {
        description: "a plain project",
        project_name: "demo",
        expected_toml_start: "name = \"demo\"\nadapter = \"duckdb\"\ndefault_target = \"dev\"\n\n[connections.developer]\ndatabase = \"demo.duckdb\"\n",
        expected_summary_start: "SQLBuild project created\n\n  Project: demo\n  Config:  sqlbuild_project.toml\n\nNext steps:\n  1. Add sources to sql/sources/\n",
    }];

    for test_case in test_cases {
        assert!(
            init_project_toml(test_case.project_name).starts_with(test_case.expected_toml_start),
            "{}",
            test_case.description
        );
        assert!(
            render_init_summary(test_case.project_name, CliStyle { use_color: false })
                .starts_with(test_case.expected_summary_start),
            "{}",
            test_case.description
        );
    }
}

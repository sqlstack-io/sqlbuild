use crate::native_commands::_helpers::command::project_facts::{
    development_project_name, has_no_runtime_extensions,
};
use crate::native_commands::tests::helpers::write_tree;
use crate::native_commands::tests::test_types::{ProjectFactsTestCase, ProjectNameTestCase};

const PROJECT: &str = "name = \"shop\"\nadapter = \"duckdb\"\n";
const NAMED_SINK_PROJECT: &str = "name = \"shop\"\nadapter = \"duckdb\"\n\n[sinks.lifecycle.named.audit]\nmin_severity = \"error\"\n";

#[test]
fn given_project_files_when_checking_runtime_extensions_then_python_owned_ones_need_python() {
    let test_cases = [
        ProjectFactsTestCase {
            description: "no project file",
            directories: &[],
            files: &[],
            expected_native: true,
        },
        ProjectFactsTestCase {
            description: "a plain project",
            directories: &[],
            files: &[("sqlbuild_project.toml", PROJECT)],
            expected_native: true,
        },
        ProjectFactsTestCase {
            description: "a sinks directory is imported",
            directories: &["sinks"],
            files: &[("sqlbuild_project.toml", PROJECT)],
            expected_native: false,
        },
        ProjectFactsTestCase {
            description: "the replaced event exporters directory is rejected in Python",
            directories: &["event_exporters"],
            files: &[],
            expected_native: false,
        },
        ProjectFactsTestCase {
            description: "a named lifecycle sink must be bound in Python",
            directories: &[],
            files: &[("sqlbuild_project.toml", NAMED_SINK_PROJECT)],
            expected_native: false,
        },
        ProjectFactsTestCase {
            description: "an invalid project file is reported by Python",
            directories: &[],
            files: &[("sqlbuild_project.toml", "name = \n")],
            expected_native: false,
        },
    ];

    for test_case in test_cases {
        let root = tempfile::tempdir().expect("temporary directory");
        write_tree(root.path(), test_case.directories, test_case.files);

        assert_eq!(
            has_no_runtime_extensions(root.path()),
            test_case.expected_native,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_project_files_when_naming_the_development_location_then_the_project_name_is_used() {
    let test_cases = [
        ProjectNameTestCase {
            description: "the project's name",
            project_file: PROJECT,
            expected_name: Some("shop"),
        },
        ProjectNameTestCase {
            description: "a blank name is refused",
            project_file: "name = \" \"\nadapter = \"duckdb\"\n",
            expected_name: None,
        },
        ProjectNameTestCase {
            description: "an invalid project file is refused",
            project_file: "name = \n",
            expected_name: None,
        },
    ];

    for test_case in test_cases {
        let root = tempfile::tempdir().expect("temporary directory");
        write_tree(
            root.path(),
            &[],
            &[("sqlbuild_project.toml", test_case.project_file)],
        );

        assert_eq!(
            development_project_name(root.path()).ok().as_deref(),
            test_case.expected_name,
            "{}",
            test_case.description
        );
    }
}

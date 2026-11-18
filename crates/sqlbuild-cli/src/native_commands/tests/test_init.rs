use std::path::Path;

use crate::native_commands::_helpers::command::init::run_init;
use crate::native_commands::models::InstalledPackage;
use crate::native_commands::tests::helpers::write_tree;
use crate::native_commands::tests::test_types::InitFailureTestCase;

#[cfg(unix)]
#[test]
fn given_unwritable_skill_directory_when_initialising_then_a_readable_error_names_the_path() {
    let test_cases = [InitFailureTestCase {
        description: "a file where the agents skill directory goes",
        blocking_file: ".agents",
        expected_stderr_start: "sqb init: cannot install SQLBuild skills into ",
        expected_exit_code: 1,
    }];

    for test_case in test_cases {
        let root = tempfile::tempdir().expect("temporary directory");
        let skills = root.path().join("packaged");
        write_tree(
            root.path(),
            &["packaged", "shop"],
            &[("packaged/SKILL.md", "# Skill\n")],
        );
        let project: &Path = &root.path().join("shop");
        write_tree(
            project,
            &[],
            &[(test_case.blocking_file, "not a directory\n")],
        );

        let output = run_init(
            project,
            &InstalledPackage {
                skills_root: skills,
            },
            false,
        )
        .expect(test_case.description);

        assert!(
            output.stderr.starts_with(test_case.expected_stderr_start)
                && output.stderr.contains(&project.display().to_string())
                && output.stderr.ends_with("Not a directory\n"),
            "{}: {}",
            test_case.description,
            output.stderr
        );
        assert_eq!(
            output.exit_code, test_case.expected_exit_code,
            "{}",
            test_case.description
        );
    }
}

use crate::tree::tests::helpers::globbed_paths;
use crate::tree::tests::test_types::GlobTestCase;

#[test]
fn given_project_layouts_when_globbing_then_paths_match_python_rglob_and_sort() {
    let test_cases = [
        GlobTestCase {
            description: "paths sort part by part, hidden files and directories match",
            files: &[
                "models/a-c.sql",
                "models/a/b.sql",
                "models/b.sql",
                "models/a/.x.sql",
            ],
            directories: &["models/dir.sql"],
            links: &[],
            suffix: ".sql",
            expected_paths: &[
                "models/a/.x.sql",
                "models/a/b.sql",
                "models/a-c.sql",
                "models/b.sql",
                "models/dir.sql",
            ],
        },
        GlobTestCase {
            description: "missing root finds nothing",
            files: &["seeds/a.sql"],
            directories: &[],
            links: &[],
            suffix: ".sql",
            expected_paths: &[],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            globbed_paths(&test_case),
            test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
}

#[cfg(unix)]
#[test]
fn given_a_directory_link_when_globbing_then_it_matches_by_name_but_is_not_walked() {
    let test_cases = [GlobTestCase {
        description: "a directory link matches by name but is not walked",
        files: &["elsewhere/inner.sql", "models/x.txt"],
        directories: &[],
        links: &[("models/linked.sql", "elsewhere")],
        suffix: ".sql",
        expected_paths: &["models/linked.sql"],
    }];
    for test_case in test_cases {
        assert_eq!(
            globbed_paths(&test_case),
            test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
}

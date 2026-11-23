use crate::tree::tests::helpers::globbed_paths;
use crate::tree::tests::test_types::GlobTestCase;

#[test]
fn given_project_layouts_when_globbing_then_paths_match_python_rglob_and_sort() {
    let test_cases = [
        GlobTestCase {
            description: "paths sort part by part, hidden files and directories match",
            files: &[
                "sql/models/a-c.sql",
                "sql/models/a/b.sql",
                "sql/models/b.sql",
                "sql/models/a/.x.sql",
            ],
            directories: &["sql/models/dir.sql"],
            links: &[],
            suffix: ".sql",
            expected_paths: &[
                "sql/models/a/.x.sql",
                "sql/models/a/b.sql",
                "sql/models/a-c.sql",
                "sql/models/b.sql",
                "sql/models/dir.sql",
            ],
        },
        GlobTestCase {
            description: "missing root finds nothing",
            files: &["sql/seeds/a.sql"],
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
        files: &["elsewhere/inner.sql", "sql/models/x.txt"],
        directories: &[],
        links: &[("sql/models/linked.sql", "elsewhere")],
        suffix: ".sql",
        expected_paths: &["sql/models/linked.sql"],
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

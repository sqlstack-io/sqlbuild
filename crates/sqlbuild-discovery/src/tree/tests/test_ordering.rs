use crate::tree::_helpers::ordering::compare_paths;
use crate::tree::tests::test_types::PathOrderTestCase;

#[test]
fn given_paths_when_sorting_then_order_matches_python_path_order_per_platform() {
    let paths: &[&str] = &[
        "models/b.sql",
        "models/A.sql",
        "models/a-c.sql",
        "models/a/b.sql",
    ];
    let test_cases = [
        PathOrderTestCase {
            description: "PurePosixPath compares parts by code point",
            case_insensitive: false,
            paths,
            expected_paths: &[
                "models/A.sql",
                "models/a/b.sql",
                "models/a-c.sql",
                "models/b.sql",
            ],
        },
        PathOrderTestCase {
            description: "PureWindowsPath compares lowercased parts",
            case_insensitive: true,
            paths,
            expected_paths: &[
                "models/a/b.sql",
                "models/a-c.sql",
                "models/A.sql",
                "models/b.sql",
            ],
        },
        PathOrderTestCase {
            description: "lowercasing is Unicode-aware",
            case_insensitive: true,
            paths: &["models/É.sql", "models/f.sql", "models/é/x.sql"],
            expected_paths: &["models/f.sql", "models/é/x.sql", "models/É.sql"],
        },
    ];
    for test_case in test_cases {
        let mut sorted: Vec<&str> = test_case.paths.to_vec();
        sorted.sort_by(|left, right| compare_paths(left, right, test_case.case_insensitive));
        assert_eq!(
            sorted, test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
}

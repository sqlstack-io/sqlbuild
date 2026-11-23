use crate::scope_index::main::posix_parent_text::posix_parent_text;
use crate::scope_index::main::posix_path_text::posix_path_text;
use crate::scope_index::tests::test_types::PosixPathTestCase;

#[test]
fn given_slash_separated_paths_when_printed_then_they_match_python_as_posix() {
    let test_cases = [
        PosixPathTestCase {
            description: "empty and current components are dropped",
            path: "sql/models//./orders.sql",
            expected_text: "sql/models/orders.sql",
            expected_parent: "models",
        },
        PosixPathTestCase {
            description: "an empty path prints as the current folder",
            path: "",
            expected_text: ".",
            expected_parent: ".",
        },
        PosixPathTestCase {
            description: "an absolute path keeps its root",
            path: "/models/orders.sql",
            expected_text: "/models/orders.sql",
            expected_parent: "/models",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            (
                posix_path_text(test_case.path),
                posix_parent_text(test_case.path)
            ),
            (
                test_case.expected_text.to_owned(),
                test_case.expected_parent.to_owned()
            ),
            "{}",
            test_case.description
        );
    }
}

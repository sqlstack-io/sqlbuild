use std::path::Path;

use crate::project_tree::main::project_path_escapes::project_path_escapes;
use crate::project_tree::tests::test_types::EscapesTestCase;

#[test]
fn given_paths_when_checking_for_escapes_then_absolute_and_parent_paths_escape() {
    let test_cases = [
        EscapesTestCase {
            description: "a relative file",
            path: "models/a.sql",
            expected_escapes: false,
        },
        EscapesTestCase {
            description: "current and empty parts",
            path: "./a//b/",
            expected_escapes: false,
        },
        EscapesTestCase {
            description: "an empty path",
            path: "",
            expected_escapes: false,
        },
        EscapesTestCase {
            description: "the current directory",
            path: ".",
            expected_escapes: false,
        },
        EscapesTestCase {
            description: "an absolute path",
            path: "/abs/x.yml",
            expected_escapes: true,
        },
        EscapesTestCase {
            description: "a double-slash root",
            path: "//x",
            expected_escapes: true,
        },
        EscapesTestCase {
            description: "a leading parent",
            path: "../x.sql",
            expected_escapes: true,
        },
        EscapesTestCase {
            description: "an inner parent",
            path: "a/../x.sql",
            expected_escapes: true,
        },
        EscapesTestCase {
            description: "only a parent",
            path: "..",
            expected_escapes: true,
        },
        EscapesTestCase {
            description: "dots inside names",
            path: "a..b/..c",
            expected_escapes: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            project_path_escapes(Path::new(test_case.path)),
            test_case.expected_escapes,
            "{}",
            test_case.description
        );
    }
}

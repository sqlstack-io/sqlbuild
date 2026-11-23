use crate::posix_path::main::posix_abspath::posix_abspath;
use crate::posix_path::main::posix_join::posix_join;
use crate::posix_path::main::posix_name::posix_name;
use crate::posix_path::main::posix_parent::posix_parent;
use crate::posix_path::main::posix_parts::posix_parts;
use crate::posix_path::main::posix_suffix::posix_suffix;
use crate::posix_path::tests::test_types::{PosixAbspathTestCase, PosixPathTestCase};

#[test]
fn given_paths_when_reading_pure_posix_parts_then_they_match_pathlib() {
    let test_cases = [
        PosixPathTestCase {
            description: "a relative file path",
            path: "models/a.sql",
            expected_parts: &["models", "a.sql"],
            expected_parent: "models",
            expected_name: "a.sql",
            expected_suffix: ".sql",
        },
        PosixPathTestCase {
            description: "current, empty and trailing components are dropped",
            path: "./a//b/",
            expected_parts: &["a", "b"],
            expected_parent: "a",
            expected_name: "b",
            expected_suffix: "",
        },
        PosixPathTestCase {
            description: "an absolute path keeps its root",
            path: "/abs/x.yml",
            expected_parts: &["/", "abs", "x.yml"],
            expected_parent: "/abs",
            expected_name: "x.yml",
            expected_suffix: ".yml",
        },
        PosixPathTestCase {
            description: "exactly two leading slashes are their own root",
            path: "//net/x",
            expected_parts: &["//", "net", "x"],
            expected_parent: "//net",
            expected_name: "x",
            expected_suffix: "",
        },
        PosixPathTestCase {
            description: "three leading slashes collapse to one root",
            path: "///three/x",
            expected_parts: &["/", "three", "x"],
            expected_parent: "/three",
            expected_name: "x",
            expected_suffix: "",
        },
        PosixPathTestCase {
            description: "the current directory has no parts",
            path: ".",
            expected_parts: &[],
            expected_parent: ".",
            expected_name: "",
            expected_suffix: "",
        },
        PosixPathTestCase {
            description: "empty text is the current directory",
            path: "",
            expected_parts: &[],
            expected_parent: ".",
            expected_name: "",
            expected_suffix: "",
        },
        PosixPathTestCase {
            description: "a parent component is kept",
            path: "a/..",
            expected_parts: &["a", ".."],
            expected_parent: "a",
            expected_name: "..",
            expected_suffix: "",
        },
        PosixPathTestCase {
            description: "a leading-dot name has a suffix only before Python 3.14",
            path: "..a",
            expected_parts: &["..a"],
            expected_parent: ".",
            expected_name: "..a",
            expected_suffix: "",
        },
        PosixPathTestCase {
            description: "a trailing dot is a suffix only from Python 3.14",
            path: "d/a.",
            expected_parts: &["d", "a."],
            expected_parent: "d",
            expected_name: "a.",
            expected_suffix: ".",
        },
        PosixPathTestCase {
            description: "the last dot starts the suffix",
            path: "d/...b.c",
            expected_parts: &["d", "...b.c"],
            expected_parent: "d",
            expected_name: "...b.c",
            expected_suffix: ".c",
        },
        PosixPathTestCase {
            description: "non-ASCII components are whole parts",
            path: "caf\u{e9}/\u{1F600}.sql",
            expected_parts: &["caf\u{e9}", "\u{1F600}.sql"],
            expected_parent: "caf\u{e9}",
            expected_name: "\u{1F600}.sql",
            expected_suffix: ".sql",
        },
        PosixPathTestCase {
            description: "a bare root is its own parent",
            path: "/",
            expected_parts: &["/"],
            expected_parent: "/",
            expected_name: "",
            expected_suffix: "",
        },
    ];
    for test_case in test_cases {
        let parts: Vec<&str> = posix_parts(test_case.path);

        assert_eq!(
            (
                parts.as_slice(),
                posix_parent(test_case.path),
                posix_name(test_case.path),
                posix_suffix(test_case.path),
            ),
            (
                test_case.expected_parts,
                test_case.expected_parent.to_owned(),
                test_case.expected_name,
                test_case.expected_suffix,
            ),
            "{}",
            test_case.description
        );
        assert_eq!(
            posix_parts(&posix_join(&parts)),
            parts,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_relative_and_absolute_paths_when_making_them_absolute_then_they_match_posixpath_abspath() {
    let test_cases = [
        PosixAbspathTestCase {
            description: "a relative artifact path",
            cwd: "/home/u/proj",
            path: "target/compiled/a.sql",
            expected_path: "/home/u/proj/target/compiled/a.sql",
        },
        PosixAbspathTestCase {
            description: "an absolute path ignores the working directory",
            cwd: "/home/u/proj",
            path: "/abs/x.sql",
            expected_path: "/abs/x.sql",
        },
        PosixAbspathTestCase {
            description: "current and empty components are dropped",
            cwd: "/home/u/proj",
            path: "./a//b/",
            expected_path: "/home/u/proj/a/b",
        },
        PosixAbspathTestCase {
            description: "a parent component climbs out of the working directory",
            cwd: "/home/u/proj",
            path: "../up/x",
            expected_path: "/home/u/up/x",
        },
        PosixAbspathTestCase {
            description: "parents stop at the root",
            cwd: "/home/u/proj",
            path: "a/../../../../x",
            expected_path: "/x",
        },
        PosixAbspathTestCase {
            description: "a root working directory",
            cwd: "/",
            path: "rel",
            expected_path: "/rel",
        },
        PosixAbspathTestCase {
            description: "a parent of the root stays at the root",
            cwd: "/",
            path: "../x",
            expected_path: "/x",
        },
        PosixAbspathTestCase {
            description: "a working directory with a trailing slash",
            cwd: "/home/u/",
            path: "x",
            expected_path: "/home/u/x",
        },
        PosixAbspathTestCase {
            description: "exactly two leading slashes are kept",
            cwd: "/home/u/proj",
            path: "//net/x",
            expected_path: "//net/x",
        },
        PosixAbspathTestCase {
            description: "three leading slashes collapse to one",
            cwd: "/home/u/proj",
            path: "///three/x",
            expected_path: "/three/x",
        },
        PosixAbspathTestCase {
            description: "a double-slash working directory keeps its root",
            cwd: "//net/share",
            path: "x/../y",
            expected_path: "//net/share/y",
        },
        PosixAbspathTestCase {
            description: "an empty path is the working directory",
            cwd: "/home/u/proj",
            path: "",
            expected_path: "/home/u/proj",
        },
        PosixAbspathTestCase {
            description: "a dot is the working directory",
            cwd: "/home/u/proj",
            path: ".",
            expected_path: "/home/u/proj",
        },
        PosixAbspathTestCase {
            description: "dots between components are dropped",
            cwd: "/home/u/proj",
            path: "a/./b/./",
            expected_path: "/home/u/proj/a/b",
        },
        PosixAbspathTestCase {
            description: "climbing past the root gives the root",
            cwd: "/home/u/proj",
            path: "/a/b/../../..",
            expected_path: "/",
        },
        PosixAbspathTestCase {
            description: "climbing past a double-slash root keeps it",
            cwd: "/home/u/proj",
            path: "//a/../..",
            expected_path: "//",
        },
        PosixAbspathTestCase {
            description: "three dots are an ordinary name",
            cwd: "/home/u/proj",
            path: "x/.../y",
            expected_path: "/home/u/proj/x/.../y",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            posix_abspath(test_case.cwd, test_case.path),
            test_case.expected_path,
            "{}",
            test_case.description
        );
    }
}

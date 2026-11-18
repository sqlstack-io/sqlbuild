use std::path::{Path, PathBuf};

use crate::native_commands::_helpers::command::paths::{pathlib_path, resolved_path};
use crate::native_commands::tests::test_types::{PathlibTestCase, ResolveTestCase};

#[test]
fn given_project_dir_text_when_building_a_path_then_pathlib_normalisation_applies() {
    let test_cases = [
        PathlibTestCase {
            description: "a leading dot and a trailing slash drop",
            text: "./shop/",
            expected_path: "shop",
        },
        PathlibTestCase {
            description: "inner dots and repeated slashes drop",
            text: "shop/.//orders",
            expected_path: "shop/orders",
        },
        PathlibTestCase {
            description: "a lone dot stays",
            text: ".",
            expected_path: ".",
        },
        PathlibTestCase {
            description: "parent references stay",
            text: "../shop",
            expected_path: "../shop",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            pathlib_path(Path::new(test_case.text)),
            PathBuf::from(test_case.expected_path),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_relative_path_when_resolving_then_existing_prefix_is_real_and_rest_is_kept() {
    let test_cases = [
        ResolveTestCase {
            description: "a missing directory is joined as written",
            text: "new/shop",
            expected_parts: &["new", "shop"],
        },
        ResolveTestCase {
            description: "a parent reference after a missing directory removes it",
            text: "missing/../new",
            expected_parts: &["new"],
        },
        ResolveTestCase {
            description: "the working directory itself",
            text: ".",
            expected_parts: &[],
        },
    ];

    for test_case in test_cases {
        let root = tempfile::tempdir().expect("temporary directory");
        let real: PathBuf = std::fs::canonicalize(root.path()).expect("real root");

        let resolved: PathBuf = resolved_path(Path::new(test_case.text), &real);

        assert_eq!(
            resolved,
            test_case
                .expected_parts
                .iter()
                .fold(real.clone(), |path, part| path.join(part)),
            "{}",
            test_case.description
        );
    }
}

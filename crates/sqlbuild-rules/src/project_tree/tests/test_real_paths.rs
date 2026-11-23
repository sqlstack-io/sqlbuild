use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::project_tree::main::project_real_paths::project_real_paths;
use crate::project_tree::tests::helpers::linked_project;
use crate::project_tree::tests::test_types::RealPathsTestCase;

#[test]
fn given_paths_resolved_together_when_resolving_then_each_resolves_as_alone() {
    let test_cases = [
        RealPathsTestCase {
            description: "plain file",
            path: "models/marts/orders.sql",
            expected_resolved: "models/marts/orders.sql",
        },
        RealPathsTestCase {
            description: "linked file",
            path: "models/link.sql",
            expected_resolved: "models/marts/orders.sql",
        },
        RealPathsTestCase {
            description: "file below a linked folder",
            path: "models/linkdir/orders.sql",
            expected_resolved: "models/marts/orders.sql",
        },
        RealPathsTestCase {
            description: "file linked out of its folder",
            path: "models/readme.sql",
            expected_resolved: "readme.md",
        },
        RealPathsTestCase {
            description: "missing file",
            path: "models/missing.sql",
            expected_resolved: "models/missing.sql",
        },
        RealPathsTestCase {
            description: "parent part before a link",
            path: "models/marts/../link.sql",
            expected_resolved: "models/marts/orders.sql",
        },
        RealPathsTestCase {
            description: "trailing current part",
            path: "models/marts/.",
            expected_resolved: "models/marts",
        },
        RealPathsTestCase {
            description: "trailing current part after a linked folder",
            path: "models/linkdir/.",
            expected_resolved: "models/marts",
        },
        RealPathsTestCase {
            description: "linked folder with a trailing slash",
            path: "models/linkdir/",
            expected_resolved: "models/marts",
        },
        RealPathsTestCase {
            description: "parent part after a linked folder",
            path: "models/linkdir/../readme.sql",
            expected_resolved: "readme.md",
        },
        RealPathsTestCase {
            description: "file below a linked project root",
            path: "sub/root_link/models/marts/orders.sql",
            expected_resolved: "models/marts/orders.sql",
        },
    ];
    let (_directory, root) = linked_project();
    std::os::unix::fs::symlink(&root, root.join("sub/root_link")).expect("root link");
    let paths: Vec<PathBuf> = test_cases
        .iter()
        .map(|test_case| root.join(test_case.path))
        .collect();
    let borrowed: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();

    let resolved: HashMap<&str, PathBuf> = test_cases
        .iter()
        .map(|test_case| test_case.path)
        .zip(project_real_paths(&borrowed))
        .collect();

    for test_case in test_cases {
        assert_eq!(
            resolved[test_case.path],
            root.join(test_case.expected_resolved),
            "{}",
            test_case.description
        );
    }
}

use std::path::Path;

use crate::project_tree::main::project_tree_children::project_tree_children;
use crate::project_tree::main::project_tree_descendants::project_tree_descendants;
use crate::project_tree::main::project_tree_glob::project_tree_glob;
use crate::project_tree::main::tracked_text_path::tracked_text_path;
use crate::project_tree::tests::test_types::{PathSelectionTestCase, TrackedTextTestCase};

const VALUES: [&str; 6] = [
    "models",
    "models/marts",
    "models/marts/orders.sql",
    "models/a.sql",
    "rules",
    "readme.md",
];

#[test]
fn given_listed_paths_when_selecting_then_children_descendants_and_globs_match_pathlib() {
    let test_cases = [
        PathSelectionTestCase {
            description: "the project root",
            argument: None,
            expected_children: vec!["models", "rules", "readme.md"],
            expected_descendants: VALUES.to_vec(),
            expected_glob: vec![],
        },
        PathSelectionTestCase {
            description: "a folder spelled with dots and slashes",
            argument: Some("./models//"),
            expected_children: vec!["models/marts", "models/a.sql"],
            expected_descendants: vec!["models/marts", "models/marts/orders.sql", "models/a.sql"],
            expected_glob: vec![],
        },
        PathSelectionTestCase {
            description: "a star pattern crossing folders",
            argument: Some("*.sql"),
            expected_children: vec![],
            expected_descendants: vec![],
            expected_glob: vec!["models/marts/orders.sql", "models/a.sql"],
        },
        PathSelectionTestCase {
            description: "a negated set",
            argument: Some("[!m]*"),
            expected_children: vec![],
            expected_descendants: vec![],
            expected_glob: vec!["rules", "readme.md"],
        },
    ];
    let values: Vec<&Path> = VALUES.iter().map(Path::new).collect();
    let pick = |indexes: Vec<usize>| -> Vec<&str> {
        indexes.into_iter().map(|index| VALUES[index]).collect()
    };
    for test_case in test_cases {
        let argument: &Path = Path::new(test_case.argument.unwrap_or(""));
        assert_eq!(
            (
                pick(project_tree_children(&values, argument)),
                pick(project_tree_descendants(
                    &values,
                    test_case.argument.map(Path::new)
                )),
                pick(project_tree_glob(&values, argument)),
            ),
            (
                test_case.expected_children,
                test_case.expected_descendants,
                test_case.expected_glob
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_file_names_when_checking_tracked_text_then_only_known_suffixes_count() {
    let test_cases = [
        TrackedTextTestCase {
            description: "a SQL model",
            path: "models/a.sql",
            expected_tracked: true,
        },
        TrackedTextTestCase {
            description: "an uppercase suffix",
            path: "models/A.YML",
            expected_tracked: true,
        },
        TrackedTextTestCase {
            description: "a hidden name without a suffix",
            path: "models/.sql",
            expected_tracked: false,
        },
        TrackedTextTestCase {
            description: "a markdown file",
            path: "readme.md",
            expected_tracked: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            tracked_text_path(Path::new(test_case.path)),
            test_case.expected_tracked,
            "{}",
            test_case.description
        );
    }
}

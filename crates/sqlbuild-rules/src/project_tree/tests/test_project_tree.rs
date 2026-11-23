use std::path::Path;

use crate::project_tree::_helpers::project_tree_file::project_tree_file;
use crate::project_tree::errors::ProjectTreeFileError;
use crate::project_tree::main::project_tree_entries::project_tree_entries;
use crate::project_tree::tests::helpers::linked_project;
use crate::project_tree::tests::test_types::{ProjectTreeEntriesTestCase, ProjectTreeFileTestCase};

#[test]
fn given_project_when_walked_then_links_hidden_and_root_tooling_directories_are_skipped() {
    let test_cases = [ProjectTreeEntriesTestCase {
        description: "links, hidden directories and top-level logs are left out",
        expected_entries: vec![
            ("models", false),
            ("models/.dot.sql", true),
            ("models/marts", false),
            ("models/marts/orders.sql", true),
            ("readme.md", true),
            ("sub", false),
            ("sub/logs", false),
            ("sub/logs/kept.sql", true),
        ],
    }];
    for test_case in test_cases {
        let (_directory, root) = linked_project();
        let mut entries: Vec<(String, bool)> = project_tree_entries(&root)
            .into_iter()
            .map(|entry| (entry.relative.to_string_lossy().into_owned(), entry.is_file))
            .collect();
        entries.sort();
        let entries: Vec<(&str, bool)> = entries
            .iter()
            .map(|(path, is_file)| (path.as_str(), *is_file))
            .collect();
        assert_eq!(
            entries, test_case.expected_entries,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_project_paths_when_resolved_then_only_tracked_files_inside_the_project_are_readable() {
    let test_cases = [
        ProjectTreeFileTestCase {
            description: "a regular file",
            path: "models/marts/orders.sql",
            expected_file: Ok("models/marts/orders.sql"),
        },
        ProjectTreeFileTestCase {
            description: "current, empty and trailing components",
            path: "./models//marts/orders.sql/",
            expected_file: Ok("models/marts/orders.sql"),
        },
        ProjectTreeFileTestCase {
            description: "a linked file resolves to its target",
            path: "models/link.sql",
            expected_file: Ok("models/marts/orders.sql"),
        },
        ProjectTreeFileTestCase {
            description: "a file under a linked directory",
            path: "models/linkdir/orders.sql",
            expected_file: Ok("models/marts/orders.sql"),
        },
        ProjectTreeFileTestCase {
            description: "a link to an untracked file",
            path: "models/readme.sql",
            expected_file: Err(ProjectTreeFileError::Untracked),
        },
        ProjectTreeFileTestCase {
            description: "an untracked file type",
            path: "readme.md",
            expected_file: Err(ProjectTreeFileError::Untracked),
        },
        ProjectTreeFileTestCase {
            description: "a missing file",
            path: "missing.sql",
            expected_file: Err(ProjectTreeFileError::Missing),
        },
        ProjectTreeFileTestCase {
            description: "a directory",
            path: "models",
            expected_file: Err(ProjectTreeFileError::Missing),
        },
        ProjectTreeFileTestCase {
            description: "a path above the project",
            path: "../outside.sql",
            expected_file: Err(ProjectTreeFileError::Escapes),
        },
        ProjectTreeFileTestCase {
            description: "a NUL byte",
            path: "a\0b.sql",
            expected_file: Err(ProjectTreeFileError::EmbeddedNul),
        },
    ];
    for test_case in test_cases {
        let (_directory, root) = linked_project();
        let resolved: Result<String, ProjectTreeFileError> =
            project_tree_file(&root, Path::new(test_case.path)).map(|file| {
                file.strip_prefix(&root)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .into_owned()
            });
        assert_eq!(
            resolved.as_ref().map(String::as_str).map_err(Clone::clone),
            test_case.expected_file,
            "{}",
            test_case.description
        );
    }
}

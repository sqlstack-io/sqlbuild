use std::path::{Path, PathBuf};

use crate::native_commands::_helpers::command::paths::{
    location_files, location_output_folders, location_watch, pathlib_path, resolved_path,
};
use crate::native_commands::models::LocationWatch;
use crate::native_commands::tests::helpers::write_tree;
use crate::native_commands::tests::test_types::{
    OutputWatchTestCase, PathlibTestCase, ResolveTestCase,
};

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

#[test]
fn given_a_dev_run_output_folder_when_a_file_is_written_then_the_watch_leaves_out_only_that_folder()
{
    let test_cases = [
        OutputWatchTestCase {
            description: "run output in the resolved folder is not an edit",
            folder: Some("output"),
            written: "output/runs/run/claim/000000000000.zst",
            expected_seen: false,
        },
        OutputWatchTestCase {
            description: "run output in a nested store folder is not an edit",
            folder: Some("state/output"),
            written: "state/output/runs/run/claim/000000000000.zst",
            expected_seen: false,
        },
        OutputWatchTestCase {
            description: "a folder given through a parent reference is resolved",
            folder: Some("sql/models/../output"),
            written: "output/runs/run/claim/000000000000.zst",
            expected_seen: false,
        },
        OutputWatchTestCase {
            description: "a source edit beside the folder is seen",
            folder: Some("output"),
            written: "sql/models/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "the folder's parent is still watched",
            folder: Some("state/output"),
            written: "state/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "another folder named output is still watched",
            folder: Some("output"),
            written: "sql/models/output/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "a folder whose name starts with output is still watched",
            folder: Some("output"),
            written: "output_models/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "a store at the project root leaves the watch on",
            folder: Some("."),
            written: "sql/models/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "a store resolving to the project root leaves the watch on",
            folder: Some("sql/models/.."),
            written: "sql/models/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "a store above the project leaves the watch on",
            folder: Some(".."),
            written: "sql/models/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "a store outside the project excludes nothing inside it",
            folder: Some("../elsewhere/output"),
            written: "output/orders.sql",
            expected_seen: true,
        },
        OutputWatchTestCase {
            description: "without a local store nothing more is left out",
            folder: None,
            written: "output/orders.sql",
            expected_seen: true,
        },
    ];

    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary directory");
        // sqb dev creates the store's folder before its location starts watching.
        write_tree(
            directory.path(),
            &["project/models", "project/state"],
            &[("project/models/orders.sql", "select 1")],
        );
        let root: PathBuf =
            std::fs::canonicalize(directory.path().join("project")).expect("real project");
        let folder: Option<PathBuf> = test_case.folder.map(|folder| root.join(folder));
        let mut watch: LocationWatch = location_watch(&root);
        watch.output_folders = location_output_folders(&root, folder.as_deref());
        let files = || {
            location_files(
                &root,
                &watch.rules,
                (&watch.local_files, &watch.output_folders),
            )
        };
        let before = files();
        let written: PathBuf = root.join(test_case.written);
        std::fs::create_dir_all(written.parent().expect("parent")).expect("create parent");
        std::fs::write(&written, "select 2 as order_id").expect("write file");

        assert_eq!(
            files() != before,
            test_case.expected_seen,
            "{}",
            test_case.description
        );
    }
}

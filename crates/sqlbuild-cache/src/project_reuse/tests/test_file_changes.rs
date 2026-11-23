use crate::project_reuse::_helpers::files::{compare_project_files, file_digest};
use crate::project_reuse::models::StoredProjectFile;
use crate::project_reuse::tests::test_types::FileComparisonTestCase;
#[cfg(unix)]
use crate::project_reuse::tests::test_types::{LinkChangeTestCase, LinkCycleTestCase};
use crate::project_snapshot::main::snapshot_project_files::snapshot_project_files;
use crate::project_snapshot::models::SnapshotRules;

#[test]
fn given_recorded_file_when_stat_is_equal_or_moves_then_racy_content_is_verified() {
    let test_cases = [
        FileComparisonTestCase {
            description: "racy equal stamps hide changed bytes",
            racy: true,
            has_digest: true,
            moved_stamp: false,
            contents: b"SELECT 2",
            expected_unchanged: false,
            expected_reads: 1,
        },
        FileComparisonTestCase {
            description: "racy equal stamps preserve unchanged bytes",
            racy: true,
            has_digest: true,
            moved_stamp: false,
            contents: b"SELECT 1",
            expected_unchanged: true,
            expected_reads: 1,
        },
        FileComparisonTestCase {
            description: "settled equal stamps are trusted",
            racy: false,
            has_digest: true,
            moved_stamp: false,
            contents: b"SELECT 2",
            expected_unchanged: true,
            expected_reads: 0,
        },
        FileComparisonTestCase {
            description: "changed stat-only record misses",
            racy: false,
            has_digest: false,
            moved_stamp: true,
            contents: b"SELECT 1",
            expected_unchanged: false,
            expected_reads: 0,
        },
        FileComparisonTestCase {
            description: "changed stamps with equal bytes reuse",
            racy: false,
            has_digest: true,
            moved_stamp: true,
            contents: b"SELECT 1",
            expected_unchanged: true,
            expected_reads: 1,
        },
    ];
    for test_case in test_cases {
        let folder = tempfile::tempdir().expect("project");
        let path = folder.path().join("orders.sql");
        std::fs::write(&path, b"SELECT 1").expect("original model");
        let mut current = snapshot_project_files(folder.path(), &SnapshotRules::default());
        let stored = vec![StoredProjectFile {
            stamp: current[0].clone(),
            digest: test_case
                .has_digest
                .then(|| file_digest(&path).expect("digest")),
            racy: test_case.racy,
        }];
        std::fs::write(&path, test_case.contents).expect("current bytes");
        current[0].mtime_ns += i64::from(test_case.moved_stamp);
        let (unchanged, _, reads) = compare_project_files(folder.path(), &stored, &current);
        assert_eq!(
            unchanged, test_case.expected_unchanged,
            "{}",
            test_case.description
        );
        assert_eq!(reads, test_case.expected_reads, "{}", test_case.description);
    }
}

#[cfg(unix)]
#[test]
fn given_linked_inputs_when_targets_change_then_compile_reuse_is_invalidated() {
    use std::os::unix::fs::symlink;

    let test_cases = [
        LinkChangeTestCase {
            description: "edit file target",
            change: |_, outside| {
                std::fs::write(outside.join("orders.sql"), b"SELECT 2").expect("edited target");
            },
            expected_unchanged: false,
        },
        LinkChangeTestCase {
            description: "retarget file link",
            change: |project, outside| {
                std::fs::remove_file(project.join("orders.sql")).expect("unlink");
                symlink(outside.join("other.sql"), project.join("orders.sql")).expect("retarget");
            },
            expected_unchanged: false,
        },
        LinkChangeTestCase {
            description: "add file through directory link",
            change: |_, outside| {
                std::fs::write(outside.join("sql/models/customers.sql"), b"SELECT 1")
                    .expect("added model");
            },
            expected_unchanged: false,
        },
        LinkChangeTestCase {
            description: "break file link",
            change: |_, outside| {
                std::fs::remove_file(outside.join("orders.sql")).expect("removed target");
            },
            expected_unchanged: false,
        },
    ];
    for test_case in test_cases {
        let project = tempfile::tempdir().expect("project");
        let outside = tempfile::tempdir().expect("linked resources");
        std::fs::create_dir_all(outside.path().join("sql/models")).expect("model directory");
        std::fs::create_dir_all(project.path().join("sql")).expect("sql directory");
        std::fs::write(outside.path().join("orders.sql"), b"SELECT 1").expect("target");
        std::fs::write(outside.path().join("other.sql"), b"SELECT 1").expect("other target");
        symlink(
            outside.path().join("orders.sql"),
            project.path().join("orders.sql"),
        )
        .expect("file link");
        symlink(
            outside.path().join("sql/models"),
            project.path().join("sql/models"),
        )
        .expect("directory link");
        let stored: Vec<StoredProjectFile> =
            snapshot_project_files(project.path(), &SnapshotRules::default())
                .into_iter()
                .map(|stamp| StoredProjectFile {
                    digest: file_digest(&project.path().join(&stamp.relative_path)),
                    stamp,
                    racy: true,
                })
                .collect();
        (test_case.change)(project.path(), outside.path());
        let current = snapshot_project_files(project.path(), &SnapshotRules::default());
        assert_eq!(
            compare_project_files(project.path(), &stored, &current).0,
            test_case.expected_unchanged,
            "{}",
            test_case.description
        );
    }
}

#[cfg(unix)]
#[test]
fn given_directory_link_to_project_root_when_snapshotting_then_cycle_is_recorded_once() {
    let test_cases = [LinkCycleTestCase {
        description: "directory link back to project root",
        expected_paths: &["loop", "orders.sql"],
    }];
    for test_case in test_cases {
        let project = tempfile::tempdir().expect("project");
        std::fs::write(project.path().join("orders.sql"), b"SELECT 1").expect("model");
        std::os::unix::fs::symlink(project.path(), project.path().join("loop")).expect("cycle");
        let snapshot = snapshot_project_files(project.path(), &SnapshotRules::default());
        let mut paths: Vec<&str> = snapshot
            .iter()
            .map(|stamp| stamp.relative_path.as_str())
            .collect();
        paths.sort_unstable();
        assert_eq!(paths, test_case.expected_paths, "{}", test_case.description);
    }
}

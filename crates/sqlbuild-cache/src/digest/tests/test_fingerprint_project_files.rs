use std::path::Path;

use crate::digest::main::fingerprint_project_files::fingerprint_project_files;
use crate::digest::tests::helpers::{EXCLUDED_MODEL, excluded_files, write_file, write_project};
use crate::digest::tests::test_types::{
    FingerprintFailureTestCase, FingerprintProjectFilesTestCase,
};

#[test]
fn given_project_edit_when_fingerprinting_then_only_covered_content_changes_it() {
    let test_cases = [
        FingerprintProjectFilesTestCase {
            description: "no edit",
            edit: |_| {},
            expected_changed: false,
        },
        FingerprintProjectFilesTestCase {
            description: "macro body edit",
            edit: |root| {
                write_file(
                    root,
                    "sql/macros/cents.py",
                    "def cents(amount):\n    return 1\n",
                )
            },
            expected_changed: true,
        },
        FingerprintProjectFilesTestCase {
            description: "helper module added",
            edit: |root| write_file(root, "sql/macros/_rounding.py", "SCALE = 2\n"),
            expected_changed: true,
        },
        FingerprintProjectFilesTestCase {
            description: "macro file removed",
            edit: |root| std::fs::remove_file(root.join("sql/macros/cents.py")).expect("removed"),
            expected_changed: true,
        },
        FingerprintProjectFilesTestCase {
            description: "file renamed with the same content",
            edit: |root| {
                std::fs::rename(root.join("sql/macros/cents.py"), root.join("sql/macros/money.py"))
                    .expect("renamed");
            },
            expected_changed: true,
        },
        FingerprintProjectFilesTestCase {
            description: "excluded model edit",
            edit: |root| write_file(root, EXCLUDED_MODEL, "SELECT 2 AS id\n"),
            expected_changed: false,
        },
        FingerprintProjectFilesTestCase {
            description: "new model is covered until the caller excludes it",
            edit: |root| write_file(root, "sql/models/customers.sql", "SELECT 1 AS id\n"),
            expected_changed: true,
        },
        FingerprintProjectFilesTestCase {
            description: "root target folder edit",
            edit: |root| write_file(root, "target/cache/store.bin", "new"),
            expected_changed: false,
        },
        FingerprintProjectFilesTestCase {
            description: "nested target folder edit",
            edit: |root| write_file(root, "sql/macros/target/notes.txt", "covered"),
            expected_changed: true,
        },
        FingerprintProjectFilesTestCase {
            description: "bytecode cache edit",
            edit: |root| write_file(root, "sql/macros/__pycache__/cents.pyc", "other"),
            expected_changed: false,
        },
        FingerprintProjectFilesTestCase {
            description: "database pages rewritten",
            edit: |root| write_file(root, "warehouse.duckdb", "other pages"),
            expected_changed: false,
        },
    ];

    for test_case in test_cases {
        let project = tempfile::tempdir().expect("temporary project");
        write_project(project.path());
        let before = fingerprint_project_files(project.path(), &excluded_files());
        (test_case.edit)(project.path());
        let after = fingerprint_project_files(project.path(), &excluded_files());

        assert!(before.is_ok(), "{}", test_case.description);
        assert_eq!(
            before.ok() != after.ok(),
            test_case.expected_changed,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_unlistable_root_when_fingerprinting_then_it_fails() {
    let test_cases = [
        FingerprintFailureTestCase {
            description: "existing project",
            root: Path::to_path_buf,
            expected_failure: false,
        },
        FingerprintFailureTestCase {
            description: "missing project",
            root: |root| root.join("missing"),
            expected_failure: true,
        },
    ];

    for test_case in test_cases {
        let project = tempfile::tempdir().expect("temporary project");
        write_project(project.path());

        let fingerprint =
            fingerprint_project_files(&(test_case.root)(project.path()), &excluded_files());

        assert_eq!(
            fingerprint.is_err(),
            test_case.expected_failure,
            "{}",
            test_case.description
        );
    }
}

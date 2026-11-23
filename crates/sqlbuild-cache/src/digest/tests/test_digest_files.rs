use std::path::PathBuf;

use crate::digest::main::digest_files::digest_files;
use crate::digest::tests::helpers::write_file;
use crate::digest::tests::test_types::DigestFilesTestCase;

#[test]
fn given_paths_when_digesting_files_then_readable_files_digest_their_bytes() {
    let test_cases = [
        DigestFilesTestCase {
            description: "regular file",
            path: |root| root.join("sql/macros/cents.py"),
            expected_digest_of: Some("def cents(amount):\n    return amount\n"),
        },
        DigestFilesTestCase {
            description: "missing file",
            path: |root| root.join("sql/macros/missing.py"),
            expected_digest_of: None,
        },
        DigestFilesTestCase {
            description: "member path inside an archive",
            path: |root| root.join("sql/macros/cents.py/inner.py"),
            expected_digest_of: None,
        },
        DigestFilesTestCase {
            description: "directory",
            path: |root| root.join("sql/macros"),
            expected_digest_of: None,
        },
    ];

    for test_case in test_cases {
        let project = tempfile::tempdir().expect("temporary project");
        write_file(
            project.path(),
            "sql/macros/cents.py",
            "def cents(amount):\n    return amount\n",
        );
        let path: PathBuf = (test_case.path)(project.path());

        let digests: Vec<Option<[u8; 32]>> =
            digest_files(&[path]).into_iter().map(Result::ok).collect();

        assert_eq!(
            digests,
            vec![
                test_case
                    .expected_digest_of
                    .map(|text| *blake3::hash(text.as_bytes()).as_bytes())
            ],
            "{}",
            test_case.description
        );
    }
}

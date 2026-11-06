use crate::artifacts::main::new_sql_test_record::new_sql_test_record;
use crate::artifacts::main::read_sql_test_cache::read_sql_test_cache;
use crate::artifacts::main::reusable_sql_test_record::reusable_sql_test_record;
use crate::artifacts::main::write_sql_test_cache::write_sql_test_cache;
use crate::artifacts::models::{SqlTestArtifactRecord, SqlTestRecordMiss};
use crate::artifacts::tests::helpers::{bump_mtime, place};
use crate::artifacts::tests::test_types::{
    ReusableRecordTestCase, SqlTestSnapshotReadTestCase, SqlTestSnapshotWriteTestCase,
};

const IDENTITY: &str = "identity-1";

#[test]
fn given_recorded_artifact_when_checking_reuse_then_reports_why_it_missed() {
    let test_cases = [
        ReusableRecordTestCase {
            description: "an unchanged artifact is kept",
            change: |_| {},
            identity: IDENTITY,
            expected_outcome: Ok(()),
        },
        ReusableRecordTestCase {
            description: "a touched artifact with the same bytes is kept",
            change: bump_mtime,
            identity: IDENTITY,
            expected_outcome: Ok(()),
        },
        ReusableRecordTestCase {
            description: "a new identity misses",
            change: |_| {},
            identity: "identity-2",
            expected_outcome: Err(SqlTestRecordMiss::IdentityChanged),
        },
        ReusableRecordTestCase {
            description: "a deleted artifact misses",
            change: |root| std::fs::remove_file(root.join("t.sql")).expect("removed"),
            identity: IDENTITY,
            expected_outcome: Err(SqlTestRecordMiss::ArtifactMissing),
        },
        ReusableRecordTestCase {
            description: "a resized artifact misses",
            change: |root| place(&root.join("t.sql"), b"select 10\n"),
            identity: IDENTITY,
            expected_outcome: Err(SqlTestRecordMiss::SizeChanged),
        },
        ReusableRecordTestCase {
            description: "same-size new bytes miss",
            change: |root| {
                place(&root.join("t.sql"), b"select 2\n");
                bump_mtime(root);
            },
            identity: IDENTITY,
            expected_outcome: Err(SqlTestRecordMiss::ContentChanged),
        },
    ];
    for test_case in test_cases {
        let root = tempfile::tempdir().expect("tests folder");
        place(&root.path().join("t.sql"), b"select 1\n");
        let record = new_sql_test_record(root.path(), &root.path().join("t.sql"), IDENTITY)
            .expect("recorded artifact");
        (test_case.change)(root.path());

        let outcome = reusable_sql_test_record(root.path(), &record, test_case.identity);

        assert_eq!(
            outcome.map(|_| ()),
            test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_native_records_when_persisting_then_preserves_python_snapshot_bytes_and_reuse() {
    let test_cases = [SqlTestSnapshotWriteTestCase {
        description: "sorted Unicode records, exact large integers and last duplicate wins",
        expected_text: concat!(
            r#"{"records":{"a":{"content_sha256":"abc","identity":"caf\u00e9 \ud83d\ude00","mtime_ns":9223372036854775807,"relative_path":"nested/orders.sql","size":18446744073709551615},"#,
            r#""z":{"content_sha256":"abc","identity":"caf\u00e9 \ud83d\ude00","mtime_ns":9223372036854775807,"relative_path":"nested/orders.sql","size":7}},"version":5}"#,
        ),
        expected_record_count: 2,
    }];
    for test_case in test_cases {
        let root = tempfile::tempdir().expect("cache folder");
        let record = SqlTestArtifactRecord {
            identity: "café 😀".to_owned(),
            relative_path: "nested/orders.sql".into(),
            size: u64::MAX,
            mtime_ns: i64::MAX,
            content_sha256: "abc".to_owned(),
        };
        let records = vec![
            ("z".to_owned(), record.clone()),
            ("a".to_owned(), record.clone()),
            (
                "z".to_owned(),
                SqlTestArtifactRecord {
                    size: 7,
                    ..record.clone()
                },
            ),
        ];

        write_sql_test_cache(root.path(), &records);

        let path = root.path().join("sql-test-artifacts.json");
        let text = std::fs::read_to_string(&path).expect("written snapshot");
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
        let loaded = read_sql_test_cache(root.path());
        assert_eq!(
            loaded.len(),
            test_case.expected_record_count,
            "{}",
            test_case.description
        );
        assert_eq!(loaded["a"], record, "{}", test_case.description);
        assert_eq!(loaded["z"].size, 7, "{}", test_case.description);

        std::fs::remove_file(&path).expect("remove snapshot");
        std::fs::create_dir(&path).expect("blocked destination");
        write_sql_test_cache(root.path(), &records);
        assert!(path.is_dir(), "{}", test_case.description);
    }
}

#[test]
fn given_mixed_or_broken_snapshots_when_reading_then_keeps_only_safe_well_typed_records() {
    let test_cases = [
        SqlTestSnapshotReadTestCase {
            description: "empty snapshot",
            contents: "",
            expected_records: &[],
        },
        SqlTestSnapshotReadTestCase {
            description: "truncated JSON",
            contents: "{",
            expected_records: &[],
        },
        SqlTestSnapshotReadTestCase {
            description: "non-object root",
            contents: "[]",
            expected_records: &[],
        },
        SqlTestSnapshotReadTestCase {
            description: "wrong version",
            contents: r#"{"version":6,"records":{}}"#,
            expected_records: &[],
        },
        SqlTestSnapshotReadTestCase {
            description: "non-object records",
            contents: r#"{"version":5,"records":[]}"#,
            expected_records: &[],
        },
        SqlTestSnapshotReadTestCase {
            description: "valid record survives bad types, ranges and paths; numeric version is compatible",
            contents: r#"{"version":5.0,"records":{
                "valid":{"identity":"orders","relative_path":"nested/orders.sql","size":11,"mtime_ns":123,"content_sha256":"abc"},
                "bool":{"identity":"orders","relative_path":"orders.sql","size":true,"mtime_ns":123,"content_sha256":"abc"},
                "float":{"identity":"orders","relative_path":"orders.sql","size":11.0,"mtime_ns":123,"content_sha256":"abc"},
                "negative":{"identity":"orders","relative_path":"orders.sql","size":-1,"mtime_ns":123,"content_sha256":"abc"},
                "overflow":{"identity":"orders","relative_path":"orders.sql","size":11,"mtime_ns":9223372036854775808,"content_sha256":"abc"},
                "type":{"identity":17,"relative_path":"orders.sql","size":11,"mtime_ns":123,"content_sha256":"abc"},
                "escape":{"identity":"orders","relative_path":"../outside.sql","size":11,"mtime_ns":123,"content_sha256":"abc"},
                "extension":{"identity":"orders","relative_path":"orders.txt","size":11,"mtime_ns":123,"content_sha256":"abc"}
            }}"#,
            expected_records: &[("orders", 11)],
        },
    ];
    for test_case in test_cases {
        let root = tempfile::tempdir().expect("cache folder");
        assert!(
            read_sql_test_cache(root.path()).is_empty(),
            "{}",
            test_case.description
        );
        std::fs::write(
            root.path().join("sql-test-artifacts.json"),
            test_case.contents,
        )
        .expect("snapshot");
        let loaded = read_sql_test_cache(root.path());
        let actual: Vec<(&str, u64)> = loaded
            .values()
            .map(|record| (record.identity.as_str(), record.size))
            .collect();
        assert_eq!(
            actual, test_case.expected_records,
            "{}",
            test_case.description
        );
    }
}

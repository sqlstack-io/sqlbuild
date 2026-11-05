use crate::fingerprint_storage::errors::{FingerprintStorageError, StorageDecodeError};
use crate::fingerprint_storage::main::decode_storage::decode_storage;
use crate::fingerprint_storage::main::insert_batch_lengths::insert_batch_lengths;
use crate::fingerprint_storage::main::insert_sql::insert_sql;
use crate::fingerprint_storage::main::latest_index::latest_index;
use crate::fingerprint_storage::models::{FingerprintIdentity, LatestIndex, StoredFingerprint};
use crate::fingerprint_storage::tests::helpers::stored_fingerprint;
use crate::fingerprint_storage::tests::test_types::{
    BatchTestCase, DecodeTestCase, InsertTestCase, LatestIndexTestCase,
};

#[test]
fn given_stored_text_when_decoding_then_python_strict_base64_and_utf8_rules_apply() {
    let test_cases = [
        DecodeTestCase {
            description: "padded multi-byte text decodes",
            storage: "4oKsQQ==",
            expected_decoded: Ok("\u{20ac}A"),
        },
        DecodeTestCase {
            description: "empty storage is empty text",
            storage: "",
            expected_decoded: Ok(""),
        },
        DecodeTestCase {
            description: "leading padding is rejected",
            storage: "=",
            expected_decoded: Err(StorageDecodeError::Base64(
                "Leading padding not allowed".to_owned(),
            )),
        },
        DecodeTestCase {
            description: "a missing pad is incorrect padding",
            storage: "QQ=",
            expected_decoded: Err(StorageDecodeError::Base64("Incorrect padding".to_owned())),
        },
        DecodeTestCase {
            description: "one data character past a quad reports the count",
            storage: "QUJDR",
            expected_decoded: Err(StorageDecodeError::Base64(
                "Invalid base64-encoded string: number of data characters (5) cannot be 1 more \
                 than a multiple of 4"
                    .to_owned(),
            )),
        },
        DecodeTestCase {
            description: "data after padding is rejected",
            storage: "QQ=A",
            expected_decoded: Err(StorageDecodeError::Base64(
                "Discontinuous padding not allowed".to_owned(),
            )),
        },
        DecodeTestCase {
            description: "a truncated sequence is an unexpected end",
            storage: "4oI=",
            expected_decoded: Err(StorageDecodeError::Utf8 {
                bytes: vec![0xe2, 0x82],
                start: 0,
                end: 2,
                reason: "unexpected end of data",
            }),
        },
        DecodeTestCase {
            description: "a surrogate lead is an invalid continuation",
            storage: "7aCA",
            expected_decoded: Err(StorageDecodeError::Utf8 {
                bytes: vec![0xed, 0xa0, 0x80],
                start: 0,
                end: 1,
                reason: "invalid continuation byte",
            }),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            decode_storage(test_case.storage).as_deref(),
            test_case.expected_decoded.as_ref().copied(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_fingerprints_when_batching_then_row_and_byte_limits_split_batches() {
    let test_cases = [
        BatchTestCase {
            description: "the row limit splits batches",
            definition_lengths: &[1, 1, 1, 1, 1],
            batch_size: 2,
            batch_max_bytes: 1_000_000,
            expected_lengths: Ok(&[2, 2, 1]),
        },
        BatchTestCase {
            description: "an oversized row still forms its own batch",
            definition_lengths: &[10, 5_000, 10],
            batch_size: 50,
            batch_max_bytes: 1_000,
            expected_lengths: Ok(&[1, 1, 1]),
        },
        BatchTestCase {
            description: "no fingerprints form no batches",
            definition_lengths: &[],
            batch_size: 50,
            batch_max_bytes: 1_000,
            expected_lengths: Ok(&[]),
        },
        BatchTestCase {
            description: "a zero batch size is rejected",
            definition_lengths: &[1],
            batch_size: 0,
            batch_max_bytes: 1_000,
            expected_lengths: Err(FingerprintStorageError::NonPositiveBatchSize),
        },
    ];
    for test_case in test_cases {
        let fingerprints: Vec<StoredFingerprint> = test_case
            .definition_lengths
            .iter()
            .map(|length| stored_fingerprint("x".repeat(*length)))
            .collect();
        assert_eq!(
            insert_batch_lengths(
                "main._sqlbuild_fingerprints",
                &fingerprints,
                test_case.batch_size,
                test_case.batch_max_bytes,
            ),
            test_case.expected_lengths.map(<[usize]>::to_vec),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_rows_when_inserting_then_literals_are_escaped_and_definitions_encoded() {
    let test_cases = [
        InsertTestCase {
            description: "one row quotes names and encodes the definition",
            definitions: &["ab"],
            expected_sql: Ok(
                "INSERT INTO main.t (node_type, node_name, target_database, target_schema, \
                 target_name, run_id, definition_hash, version_hash, schema_fingerprint, \
                 definition_b64, metadata_json_b64, ts, event_id) VALUES ('model', 'o''rders', NULL, \
                 'main', 'orders', 'run', 'd', '', 's', 'YWI=', 'e30=', '2026-01-01T00:00:00', \
                 'b9397954c803e7f99f6a233c2985be772c7ddf2c6f234197d77bc227ae175cbd')",
            ),
        },
        InsertTestCase {
            description: "an empty batch is rejected",
            definitions: &[],
            expected_sql: Err(FingerprintStorageError::EmptyInsert),
        },
    ];
    for test_case in test_cases {
        let fingerprints: Vec<StoredFingerprint> = test_case
            .definitions
            .iter()
            .map(|definition| stored_fingerprint((*definition).to_owned()))
            .collect();
        assert_eq!(
            insert_sql("main.t", &fingerprints),
            test_case.expected_sql.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_name_collisions_when_indexing_latest_rows_then_models_win_and_order_is_kept() {
    let test_cases = [
        LatestIndexTestCase {
            description: "a later model replaces an earlier name; other types do not",
            identities: &[
                ("udf", "orders"),
                ("seed", "customers"),
                ("model", "orders"),
                ("hook", "orders"),
                ("udf", "orders"),
            ],
            expected_index: LatestIndex {
                by_name: vec![2, 1],
                by_identity: vec![4, 1, 2, 3],
            },
        },
        LatestIndexTestCase {
            description: "no rows index nothing",
            identities: &[],
            expected_index: LatestIndex::default(),
        },
    ];
    for test_case in test_cases {
        let identities: Vec<FingerprintIdentity<'_>> = test_case
            .identities
            .iter()
            .map(|(node_type, node_name)| FingerprintIdentity {
                node_type,
                node_name,
            })
            .collect();
        assert_eq!(
            latest_index(&identities),
            test_case.expected_index,
            "{}",
            test_case.description
        );
    }
}

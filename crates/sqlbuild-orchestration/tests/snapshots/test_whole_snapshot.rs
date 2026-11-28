//! Snapshot bodies are read as hexadecimal text, whole snapshots are decoded once and shared, and their decoding waits for the memory budget.

use serde_json::json;

use crate::snapshots::tests::helpers::{budgeted_read, whole_snapshot_reads};
use crate::snapshots::tests::test_types::{
    DecodeBudgetTestCase, StoredBodyTestCase, WholeSnapshotReadTestCase, WholeSnapshotReads,
};
use crate::store::main::compress::compress;
use crate::store::main::decompress::decompress;
use crate::store::tests::store_helpers::run;
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_published_snapshot_when_each_reader_reads_it_then_every_body_arrives_as_hex_and_one_copy_is_shared()
 {
    let expected = WholeSnapshotReads {
        current_nodes: 2,
        current_sources: 2,
        current_documents: 1,
        shared_nodes: 2,
        selected_nodes: 1,
        one_shared_copy: true,
    };
    let test_cases = [
        WholeSnapshotReadTestCase {
            description: "SQLite snapshot readers",
            backend: Backend::Sqlite,
            expected_reads: WholeSnapshotReads { ..expected },
        },
        WholeSnapshotReadTestCase {
            description: "PostgreSQL snapshot readers",
            backend: Backend::Postgres,
            expected_reads: expected,
        },
    ];
    for test_case in test_cases {
        let Some(observed) = run(whole_snapshot_reads(test_case.backend)) else {
            continue;
        };
        assert_eq!(
            observed, test_case.expected_reads,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_stored_body_when_it_is_decoded_then_only_hexadecimal_text_is_accepted() {
    let hex: String = compress(b"orders")
        .expect("compress")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let bytes: Vec<u8> = compress(b"orders").expect("compress");
    let test_cases = [
        StoredBodyTestCase {
            description: "a body selected as hexadecimal text decodes",
            stored: json!(hex),
            expected_decoded: Ok(b"orders"),
        },
        StoredBodyTestCase {
            description: "a body selected as bytes, one JSON number per byte, is refused",
            stored: json!(bytes),
            expected_decoded: Err(
                "invalid control-plane data: a stored body must be selected as hexadecimal text, not bytes",
            ),
        },
    ];
    for test_case in test_cases {
        let observed = decompress(&test_case.stored).map_err(|error| error.to_string());
        assert_eq!(
            observed.as_deref().map_err(String::as_str),
            test_case.expected_decoded,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_spent_decode_budget_when_a_whole_snapshot_read_starts_then_it_waits_until_the_budget_frees()
 {
    let test_cases = [
        DecodeBudgetTestCase {
            description: "SQLite whole-snapshot read",
            backend: Backend::Sqlite,
            expected_wait: (false, 2),
        },
        DecodeBudgetTestCase {
            description: "PostgreSQL whole-snapshot read",
            backend: Backend::Postgres,
            expected_wait: (false, 2),
        },
    ];
    for test_case in test_cases {
        let Some(observed) = run(budgeted_read(test_case.backend)) else {
            continue;
        };
        assert_eq!(
            observed, test_case.expected_wait,
            "{}",
            test_case.description
        );
    }
}

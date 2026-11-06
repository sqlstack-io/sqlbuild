use crate::clickhouse::main::error_kind::error_kind;
use crate::clickhouse::main::parse_column_encoding::parse_column_encoding;
use crate::clickhouse::main::stored_rows_sql::stored_rows_sql;
use crate::clickhouse::models::ColumnEncoding;
use crate::clickhouse::tests::helpers::{
    BEAT_KEY, batch, batch_hash, block, encoded_heartbeat, first_mismatch, planned, stored,
};
use crate::clickhouse::tests::test_types::{
    EncodingTestCase, ErrorKindTestCase, LookupTestCase, PlanTestCase, RowBinaryTestCase,
    TtlTestCase,
};
use crate::schema::models::{DdlOptions, Table};
use crate::store::errors::StoreErrorKind;

const OPTIONS: fn() -> DdlOptions = || DdlOptions {
    database: Some("observability".to_owned()),
    retention_days: 30,
    cluster: None,
    producer_version: "9.8.7".to_owned(),
};

#[test]
fn given_column_types_when_parsed_then_recall_types_have_an_encoding() {
    let test_cases = [
        EncodingTestCase {
            description: "a plain string",
            type_text: "String",
            expected_encoding: Some((ColumnEncoding::String, false)),
        },
        EncodingTestCase {
            description: "a nullable integer",
            type_text: "Nullable(Int64)",
            expected_encoding: Some((ColumnEncoding::Int64, true)),
        },
        EncodingTestCase {
            description: "a timestamp with its zone",
            type_text: "DateTime64(6, 'UTC')",
            expected_encoding: Some((ColumnEncoding::DateTime64(6), false)),
        },
        EncodingTestCase {
            description: "a type Recall never creates",
            type_text: "Array(String)",
            expected_encoding: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_column_encoding(test_case.type_text),
            test_case.expected_encoding,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_heartbeat_when_encoded_then_the_block_is_row_binary_with_ingested_at() {
    let beat_at: [u8; 8] = 1_792_368_001_000_000_i64.to_le_bytes();
    let pid: [u8; 8] = 7_i64.to_le_bytes();
    let ingested_millis: [u8; 8] = 1_000_i64.to_le_bytes();
    let test_cases = [
        RowBinaryTestCase {
            description: "nullable columns carry a null flag and absent columns are left out",
            columns: &[
                ("beat_at", ColumnEncoding::DateTime64(6), false),
                ("host_token", ColumnEncoding::String, true),
                ("pid", ColumnEncoding::Int64, true),
                ("absent_in_rows", ColumnEncoding::String, true),
                ("ingested_at", ColumnEncoding::DateTime64(3), false),
            ],
            expected_block: Ok((
                vec![
                    "beat_at".to_owned(),
                    "host_token".to_owned(),
                    "pid".to_owned(),
                    "ingested_at".to_owned(),
                ],
                block(&[&beat_at, &[0, 4], b"host", &[0], &pid, &ingested_millis]),
            )),
        },
        RowBinaryTestCase {
            description: "a timestamp column refuses an integer",
            columns: &[("pid", ColumnEncoding::DateTime64(6), false)],
            expected_block: Err(
                "Recall column pid holds a value that is not a timestamp".to_owned()
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            encoded_heartbeat(test_case.columns, 1_000_000),
            test_case.expected_block,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_stored_rows_when_planned_then_duplicates_are_skipped_conflicts_kept_and_stale_artifacts_refreshed()
 {
    let beat_hash: String = batch_hash(Table::Heartbeats);
    let artifact_hash: String = batch_hash(Table::Artifacts);
    let test_cases = [
        PlanTestCase {
            description: "nothing stored writes every row",
            stored: vec![],
            expected_plan: (
                vec![
                    ("recall_artifact_refs", 1),
                    ("recall_artifacts", 1),
                    ("recall_heartbeats", 1),
                ],
                0,
            ),
        },
        PlanTestCase {
            description: "equal copies and a recently seen artifact are not written again",
            stored: vec![
                stored("recall_heartbeats", (BEAT_KEY, &beat_hash), None),
                stored(
                    "recall_artifacts",
                    ("sha256:abc", &artifact_hash),
                    Some("2026-10-18T00:00:00.000000Z"),
                ),
            ],
            expected_plan: (vec![("recall_artifact_refs", 1)], 0),
        },
        PlanTestCase {
            description: "an artifact last seen before the refresh cutoff is written again",
            stored: vec![stored(
                "recall_artifacts",
                ("sha256:abc", &artifact_hash),
                Some("2026-10-01T00:00:00.000000Z"),
            )],
            expected_plan: (
                vec![
                    ("recall_artifact_refs", 1),
                    ("recall_artifacts", 1),
                    ("recall_heartbeats", 1),
                ],
                0,
            ),
        },
        PlanTestCase {
            description: "a key holding other content keeps it and records a conflict",
            stored: vec![stored("recall_heartbeats", (BEAT_KEY, "other"), None)],
            expected_plan: (
                vec![
                    ("recall_artifact_refs", 1),
                    ("recall_artifacts", 1),
                    ("recall_conflicts", 1),
                ],
                1,
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            planned(&test_case.stored),
            test_case.expected_plan,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_batch_when_looked_up_then_each_table_is_filtered_by_its_keys() {
    let test_cases = [
        LookupTestCase {
            description: "heartbeats and artifacts are looked up by their keys in one query",
            sql: stored_rows_sql(&batch(), &OPTIONS()),
            expected_fragments: &[
                "FROM `observability`.`recall_heartbeats` WHERE `invocation_id` IN ('inv-1') \
                 AND `beat_at` IN (fromUnixTimestamp64Micro(1792368001000000, 'UTC'))",
                "FROM `observability`.`recall_artifacts` WHERE `digest` IN ('sha256:abc') \
                 GROUP BY record_key",
                "\nUNION ALL\n",
            ],
        },
        LookupTestCase {
            description: "an empty batch needs no lookup",
            sql: stored_rows_sql(&[], &OPTIONS()),
            expected_fragments: &[],
        },
    ];

    for test_case in test_cases {
        let sql: String = test_case.sql.clone().unwrap_or_default();
        assert_eq!(
            test_case
                .expected_fragments
                .iter()
                .filter(|fragment| !sql.contains(**fragment))
                .collect::<Vec<_>>(),
            Vec::<&&str>::new(),
            "{}: {sql}",
            test_case.description
        );
        assert_eq!(
            test_case.sql.is_some(),
            !test_case.expected_fragments.is_empty(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_failure_messages_when_classified_then_permission_and_transient_codes_are_distinguished() {
    let test_cases = [
        ErrorKindTestCase {
            description: "access denied",
            message: "Code: 497. DB::Exception: default: Not enough privileges. (ACCESS_DENIED)",
            expected_kind: StoreErrorKind::Permission,
        },
        ErrorKindTestCase {
            description: "too many parts is retried",
            message: "Received ClickHouse exception, code: 252, server response: Code: 252.",
            expected_kind: StoreErrorKind::Busy,
        },
        ErrorKindTestCase {
            description: "a network failure without a server code is retried",
            message: "Error HTTPConnectionPool(host='localhost', port=1): Max retries exceeded",
            expected_kind: StoreErrorKind::Busy,
        },
        ErrorKindTestCase {
            description: "an unknown database is reported",
            message: "Code: 81. DB::Exception: Database observability does not exist.",
            expected_kind: StoreErrorKind::NotFound,
        },
        ErrorKindTestCase {
            description: "other server errors are not retried",
            message: "Code: 62. DB::Exception: Syntax error",
            expected_kind: StoreErrorKind::Other,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            error_kind(test_case.message),
            test_case.expected_kind,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_stored_ttls_when_compared_then_tables_off_the_configured_retention_are_reported() {
    let test_cases = [
        TtlTestCase {
            description: "the configured retention matches",
            create_query: "CREATE TABLE d.recall_output (`x` String) ENGINE = ReplacingMergeTree \
                           ORDER BY x TTL invocation_started_at + toIntervalDay(30) SETTINGS a = 1",
            retention_days: 30,
            expected_stored_days: None,
        },
        TtlTestCase {
            description: "a store kept for another retention",
            create_query: "CREATE TABLE d.recall_output (`x` String) ENGINE = ReplacingMergeTree \
                           ORDER BY x TTL invocation_started_at + toIntervalDay(30) SETTINGS a = 1",
            retention_days: 7,
            expected_stored_days: Some(Some(30)),
        },
        TtlTestCase {
            description: "a table without a TTL",
            create_query: "CREATE TABLE d.recall_output (`x` String) ENGINE = MergeTree ORDER BY x",
            retention_days: 30,
            expected_stored_days: Some(None),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            first_mismatch(test_case.create_query, test_case.retention_days),
            test_case.expected_stored_days,
            "{}",
            test_case.description
        );
    }
}

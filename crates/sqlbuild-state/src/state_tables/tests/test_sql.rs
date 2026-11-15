use crate::events::constants::{
    COLUMN_MIGRATION_COLUMNS, MIGRATION_COLUMNS, NODE_RESULT_COLUMNS, OLD_NAME_VIEW_COLUMNS,
};
use crate::state_tables::main::create_table_sql::create_table_sql;
use crate::state_tables::main::read_history_sql::read_history_sql;
use crate::state_tables::main::values_insert_sql::values_insert_sql;
use crate::state_tables::models::HistoryRead;
use crate::state_tables::tests::helpers::column;
use crate::state_tables::tests::test_types::{
    CreateTableTestCase, EventKeyTestCase, HistoryReadTestCase, ValuesInsertTestCase,
};

#[test]
fn given_columns_when_rendering_ddl_then_required_columns_are_not_null() {
    let test_cases = [
        CreateTableTestCase {
            description: "mixed required columns",
            columns: vec![column("a", "VARCHAR", true), column("b", "BIGINT", false)],
            expected_sql: "CREATE TABLE IF NOT EXISTS s.t (a VARCHAR NOT NULL, b BIGINT)",
        },
        CreateTableTestCase {
            description: "no columns",
            columns: Vec::new(),
            expected_sql: "CREATE TABLE IF NOT EXISTS s.t ()",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            create_table_sql("s.t", &test_case.columns),
            test_case.expected_sql,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_rows_when_rendering_insert_then_each_row_is_one_values_tuple() {
    let test_cases = [ValuesInsertTestCase {
        description: "two rows",
        rows: vec![
            vec!["'a'".to_owned(), "NULL".to_owned()],
            vec!["'b'".to_owned(), "'c'".to_owned()],
        ],
        expected_sql: "INSERT INTO s.t (x, y) VALUES ('a', NULL), ('b', 'c')",
    }];

    for test_case in test_cases {
        assert_eq!(
            values_insert_sql("s.t", &["x", "y"], &test_case.rows),
            test_case.expected_sql,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_table_without_event_ids_when_paging_history_then_the_missing_key_never_orders_or_converts()
 {
    let test_cases = [
        HistoryReadTestCase {
            description: "Snowflake rejects TO_BINARY of a bare NULL",
            adapter: "snowflake",
            has_event_id: false,
            after: Some(vec![Some("'e'".to_owned()), Some("ts1".to_owned()), None]),
            expected_sql: "SELECT k, ts, NULL AS event_id FROM s.t WHERE ((TO_BINARY(k, 'UTF-8') > \
                TO_BINARY('e', 'UTF-8')) OR (TO_BINARY(k, 'UTF-8') = TO_BINARY('e', 'UTF-8') AND ts > ts1) \
                OR (TO_BINARY(k, 'UTF-8') = TO_BINARY('e', 'UTF-8') AND ts = ts1 AND NULL IS NOT NULL)) \
                ORDER BY CASE WHEN k IS NULL THEN 0 ELSE 1 END, TO_BINARY(k, 'UTF-8'), \
                CASE WHEN ts IS NULL THEN 0 ELSE 1 END, ts LIMIT 5",
        },
        HistoryReadTestCase {
            description: "SQL Server rejects a constant in ORDER BY",
            adapter: "sqlserver",
            has_event_id: false,
            after: None,
            expected_sql: "SELECT TOP 5 k, ts, NULL AS event_id FROM s.t ORDER BY \
                CASE WHEN k IS NULL THEN 0 ELSE 1 END, CONVERT(VARBINARY(MAX), CONVERT(NVARCHAR(MAX), k)), \
                CASE WHEN ts IS NULL THEN 0 ELSE 1 END, ts",
        },
        HistoryReadTestCase {
            description: "a real event ID stays the last ordering key",
            adapter: "snowflake",
            has_event_id: true,
            after: None,
            expected_sql: "SELECT k, ts, event_id FROM s.t ORDER BY CASE WHEN k IS NULL THEN 0 ELSE 1 END, \
                TO_BINARY(k, 'UTF-8'), CASE WHEN ts IS NULL THEN 0 ELSE 1 END, ts, \
                CASE WHEN event_id IS NULL THEN 0 ELSE 1 END, TO_BINARY(event_id, 'UTF-8') LIMIT 5",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            read_history_sql(
                "s.t",
                test_case.after.as_deref(),
                HistoryRead {
                    columns: &["k", "ts"],
                    key_columns: None,
                    timestamp_column: "ts",
                    observed_since: None,
                    adapter: test_case.adapter,
                    has_event_id: test_case.has_event_id,
                    limit: 5,
                },
            )
            .expect("supported adapter"),
            test_case.expected_sql,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_event_tables_keyed_by_event_id_when_paging_history_then_each_column_and_key_appears_once()
{
    let test_cases = [
        EventKeyTestCase {
            description: "migrations",
            columns: &MIGRATION_COLUMNS,
            key_columns: &["event_id", "created_at"],
            timestamp_column: "created_at",
            expected_ordering_keys: &["event_id", "created_at"],
        },
        EventKeyTestCase {
            description: "column migrations",
            columns: &COLUMN_MIGRATION_COLUMNS,
            key_columns: &["event_id", "created_at"],
            timestamp_column: "created_at",
            expected_ordering_keys: &["event_id", "created_at"],
        },
        EventKeyTestCase {
            description: "old-name views",
            columns: &OLD_NAME_VIEW_COLUMNS,
            key_columns: &["event_id", "created_at"],
            timestamp_column: "created_at",
            expected_ordering_keys: &["event_id", "created_at"],
        },
        EventKeyTestCase {
            description: "microbatches",
            columns: &["event_id", "record_type", "scope_kind", "created_at"],
            key_columns: &["event_id", "created_at"],
            timestamp_column: "created_at",
            expected_ordering_keys: &["event_id", "created_at"],
        },
        EventKeyTestCase {
            description: "janitor events",
            columns: &["event_id", "schema_version", "event_type", "occurred_at"],
            key_columns: &["event_id", "occurred_at"],
            timestamp_column: "occurred_at",
            expected_ordering_keys: &["event_id", "occurred_at"],
        },
        EventKeyTestCase {
            description: "node results, which carry no event ID column",
            columns: &NODE_RESULT_COLUMNS,
            key_columns: &[
                "node_type",
                "node_name",
                "target_database",
                "target_schema",
                "target_name",
                "run_id",
                "status",
                "ts",
            ],
            timestamp_column: "ts",
            expected_ordering_keys: &[
                "node_type",
                "node_name",
                "target_database",
                "target_schema",
                "target_name",
                "run_id",
                "status",
                "ts",
                "event_id",
            ],
        },
    ];
    let adapters = [
        "snowflake",
        "duckdb",
        "motherduck",
        "sqlite",
        "postgres",
        "bigquery",
        "databricks",
        "sqlserver",
        "clickhouse",
    ];
    for test_case in test_cases {
        for adapter in adapters {
            let protocol_cursor = vec![Some("'x'".to_owned()); test_case.key_columns.len() + 1];
            let sql = read_history_sql(
                "s.t",
                Some(&protocol_cursor),
                HistoryRead {
                    columns: test_case.columns,
                    key_columns: Some(test_case.key_columns),
                    timestamp_column: test_case.timestamp_column,
                    observed_since: None,
                    adapter,
                    has_event_id: true,
                    limit: 5,
                },
            )
            .expect("supported adapter");
            let head = sql.strip_prefix("SELECT ").expect("select");
            let head = head.strip_prefix("TOP 5 ").unwrap_or(head);
            let (projection, rest) = head.split_once(" FROM ").expect("from");
            let mut selected: Vec<&str> = projection.split(", ").collect();
            selected.sort_unstable();
            selected.dedup();
            assert_eq!(
                selected.len(),
                test_case.columns.len() + usize::from(!test_case.columns.contains(&"event_id")),
                "{} on {adapter}: each column selected once in {sql}",
                test_case.description
            );
            let ordering_keys: Vec<&str> = rest
                .split_once(" ORDER BY ")
                .expect("order")
                .1
                .split("CASE WHEN ")
                .skip(1)
                .filter_map(|item| item.split_once(" IS NULL").map(|(key, _)| key))
                .collect();
            assert_eq!(
                ordering_keys, test_case.expected_ordering_keys,
                "{} on {adapter}: {sql}",
                test_case.description
            );
        }
    }
}

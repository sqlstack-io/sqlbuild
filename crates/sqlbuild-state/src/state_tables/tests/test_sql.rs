use crate::state_tables::main::create_table_sql::create_table_sql;
use crate::state_tables::main::read_history_sql::read_history_sql;
use crate::state_tables::main::read_keys_sql::read_keys_sql;
use crate::state_tables::main::values_insert_sql::values_insert_sql;
use crate::state_tables::models::HistoryRead;
use crate::state_tables::tests::helpers::column;
use crate::state_tables::tests::test_types::{
    CreateTableTestCase, HistoryReadTestCase, ValuesInsertTestCase,
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
fn given_missing_keys_when_reading_rows_then_each_key_matches_its_columns_and_nulls() {
    assert_eq!(
        read_keys_sql(
            "s.t",
            (&["k", "phase", "ts"], false),
            &["k", "phase"],
            &[
                vec![Some("'a'".to_owned()), None],
                vec![Some("'b'".to_owned()), Some("'final'".to_owned())],
            ],
        ),
        "SELECT k, phase, ts, NULL AS event_id FROM s.t WHERE (k = 'a' AND phase IS NULL) \
            OR (k = 'b' AND phase = 'final')"
    );
}

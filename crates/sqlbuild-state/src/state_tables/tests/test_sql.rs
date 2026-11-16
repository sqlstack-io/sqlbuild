use crate::state_tables::main::create_table_sql::create_table_sql;
use crate::state_tables::main::read_history_sql::read_history_sql;
use crate::state_tables::main::values_insert_sql::values_insert_sql;
use crate::state_tables::models::{HistoryRead, NewestVersions};
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
                    newest: None,
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
fn given_event_id_among_the_columns_when_paging_history_then_the_cursor_key_is_aliased() {
    let sql: Result<String, String> = read_history_sql(
        "s.t",
        None,
        HistoryRead {
            columns: &["event_id", "ts"],
            key_columns: Some(&["event_id", "ts"]),
            timestamp_column: "ts",
            observed_since: None,
            adapter: "snowflake",
            has_event_id: true,
            limit: 5,
            newest: None,
        },
    );

    assert_eq!(
        sql,
        Ok(
            "SELECT event_id, ts, event_id AS sqb_event_id_key FROM s.t ORDER BY \
            CASE WHEN event_id IS NULL THEN 0 ELSE 1 END, TO_BINARY(event_id, 'UTF-8'), \
            CASE WHEN ts IS NULL THEN 0 ELSE 1 END, ts, \
            CASE WHEN event_id IS NULL THEN 0 ELSE 1 END, TO_BINARY(event_id, 'UTF-8') LIMIT 5"
                .to_owned()
        ),
        "each output column name appears once, so Snowflake can resolve ORDER BY event_id"
    );
}

#[test]
fn given_a_version_limit_when_reading_history_then_only_each_identitys_newest_rows_are_selected() {
    let sql: Result<String, String> = read_history_sql(
        "s.t",
        None,
        HistoryRead {
            columns: &["node_name", "run_id", "ts"],
            key_columns: None,
            timestamp_column: "ts",
            observed_since: None,
            adapter: "duckdb",
            has_event_id: true,
            limit: 5,
            newest: Some(NewestVersions {
                partition: &["node_name"],
                tie_breakers: &["run_id"],
                versions: 3,
            }),
        },
    );

    assert_eq!(
        sql,
        Ok(
            "SELECT node_name, run_id, ts, event_id FROM (SELECT *, ROW_NUMBER() OVER \
            (PARTITION BY node_name ORDER BY ts DESC, run_id DESC, event_id DESC) AS sqb_version \
            FROM s.t) AS sqb_versions WHERE sqb_version <= 3 ORDER BY \
            CASE WHEN node_name IS NULL THEN 0 ELSE 1 END, encode(node_name), \
            CASE WHEN run_id IS NULL THEN 0 ELSE 1 END, encode(run_id), \
            CASE WHEN ts IS NULL THEN 0 ELSE 1 END, ts, \
            CASE WHEN event_id IS NULL THEN 0 ELSE 1 END, encode(event_id) LIMIT 5"
                .to_owned()
        ),
        "the rank keeps the newest three rows of each node and is never projected"
    );
}

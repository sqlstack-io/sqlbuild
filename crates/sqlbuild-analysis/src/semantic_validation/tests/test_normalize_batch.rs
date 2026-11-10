use std::collections::HashMap;

use crate::semantic_validation::main::normalize_batch::normalize_analysis_sqls;
use crate::semantic_validation::main::normalize_native::normalize_native_sql;
use crate::semantic_validation::tests::test_types::{
    NativeStubTestCase, NormalizationBatchTestCase,
};
use crate::semantic_validation::types::NormalizationRequest;

#[test]
fn given_batch_when_normalizing_on_pool_or_in_turn_then_matches_single_calls_in_order() {
    let test_cases = [
        NormalizationBatchTestCase {
            description: "stubbed references and placeholders",
            dialect: "snowflake",
            requests: &[
                (
                    r#"SELECT * FROM __ref("orders") JOIN __ref("customers") USING (id)"#,
                    &[("orders", "__sqb_rel_0"), ("customers", "__sqb_rel_1")],
                ),
                ("SELECT payload:item[0] AS item FROM inventory", &[]),
                ("SELECT 1 LIMIT @@@limit", &[]),
            ],
            expected_failures: &[],
        },
        NormalizationBatchTestCase {
            description: "failures stay at their own positions",
            dialect: "snowflake",
            requests: &[
                ("SELECT 'unterminated FROM orders", &[]),
                ("SELECT id FROM products", &[]),
                ("SELECT /* unterminated FROM customers", &[]),
            ],
            expected_failures: &[0, 2],
        },
        NormalizationBatchTestCase {
            description: "double-slash comments hide quotes and brackets",
            dialect: "snowflake",
            requests: &[
                ("SELECT id // it's the key\nFROM orders", &[]),
                (
                    "// don't read \"items[0\nSELECT payload:item[0] AS item FROM inventory",
                    &[],
                ),
                ("SELECT id FROM orders // trailing 'note", &[]),
            ],
            expected_failures: &[],
        },
    ];
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(3)
        .build()
        .expect("test pool must build");

    for test_case in test_cases {
        let requests = || -> Vec<NormalizationRequest> {
            test_case
                .requests
                .iter()
                .map(|(sql, stubs)| {
                    (
                        (*sql).to_owned(),
                        stubs
                            .iter()
                            .map(|(name, stub)| ((*name).to_owned(), (*stub).to_owned()))
                            .collect(),
                        HashMap::from([("limit".to_owned(), "10".to_owned())]),
                    )
                })
                .collect()
        };
        let expected: Vec<Result<String, String>> = requests()
            .into_iter()
            .map(|(sql, stubs, placeholders)| {
                crate::semantic_validation::_helpers::normalization::normalize_analysis_sql(
                    &sql,
                    test_case.dialect,
                    stubs,
                    placeholders,
                )
            })
            .collect();
        let failures: Vec<usize> = (0..expected.len())
            .filter(|index| expected[*index].is_err())
            .collect();

        assert_eq!(
            failures, test_case.expected_failures,
            "{}",
            test_case.description
        );
        assert_eq!(
            normalize_analysis_sqls(test_case.dialect, requests(), Some(&pool)),
            expected,
            "{}",
            test_case.description
        );
        assert_eq!(
            normalize_analysis_sqls(test_case.dialect, requests(), None),
            expected,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_cursor_stubs_when_normalizing_for_a_native_parser_then_only_whole_names_in_code_are_respelt()
 {
    let test_cases = [
        NativeStubTestCase {
            dialect: "postgres",
            description: "a cursor stub in code takes the dialect's prefix",
            sql: "SELECT * FROM t WHERE ts > __cursor_start()",
            expected_sql: "SELECT * FROM t WHERE ts > __sqlbuild.cursor_start()",
        },
        NativeStubTestCase {
            dialect: "postgres",
            description: "a cursor stub's text inside a string literal is left as written",
            sql: "SELECT '__cursor_start()' AS s FROM t",
            expected_sql: "SELECT '__cursor_start()' AS s FROM t",
        },
        NativeStubTestCase {
            dialect: "postgres",
            description: "a cursor stub's text inside a comment is left as written",
            sql: "SELECT 1 FROM t -- __cursor_end()\n",
            expected_sql: "SELECT 1 FROM t -- __cursor_end()\n",
        },
        NativeStubTestCase {
            dialect: "postgres",
            description: "a longer name ending in a cursor stub's text is another function",
            sql: "SELECT x__cursor_end() FROM t",
            expected_sql: "SELECT x__cursor_end() FROM t",
        },
        NativeStubTestCase {
            dialect: "postgres",
            description: "a quoted stub beside a stub in code leaves only the code one respelt",
            sql: "SELECT '__cursor_end()', __cursor_end() FROM t",
            expected_sql: "SELECT '__cursor_end()', __sqlbuild.cursor_end() FROM t",
        },
        NativeStubTestCase {
            dialect: "duckdb",
            description: "a DuckDB backslash before a closing quote is literal, so the stub after it is code",
            sql: r"SELECT 'C:\' AS folder, __cursor_end() FROM t",
            expected_sql: r"SELECT 'C:\' AS folder, __sqlbuild.cursor_end() FROM t",
        },
        NativeStubTestCase {
            dialect: "postgres",
            description: "a Postgres escape string's backslash-quoted quote keeps the stub after it quoted",
            sql: r"SELECT E'O\'Brien __cursor_end()', __cursor_end() FROM t",
            expected_sql: r"SELECT E'O\'Brien __cursor_end()', __sqlbuild.cursor_end() FROM t",
        },
        NativeStubTestCase {
            dialect: "postgres",
            description: "a plain Postgres string's backslash is literal, so the stub after it is code",
            sql: r"SELECT 'C:\', __cursor_end() FROM t",
            expected_sql: r"SELECT 'C:\', __sqlbuild.cursor_end() FROM t",
        },
        NativeStubTestCase {
            dialect: "snowflake",
            description: "a Snowflake backslash-quoted quote keeps the stub after it quoted",
            sql: r"SELECT 'it\'s __cursor_end()', __cursor_end() FROM t",
            expected_sql: r"SELECT 'it\'s __cursor_end()', __sqlbuild.cursor_end() FROM t",
        },
        NativeStubTestCase {
            dialect: "tsql",
            description: "a T-SQL bracket name's apostrophe opens no string, so the stub after it is code",
            sql: "SELECT [customer's name] AS n FROM t WHERE ts >= __cursor_start()",
            expected_sql: "SELECT [customer's name] AS n FROM t WHERE ts >= __sqlbuild.cursor_start()",
        },
        NativeStubTestCase {
            dialect: "clickhouse",
            description: "a ClickHouse backslash-quoted quote in an identifier keeps the stub after it in code",
            sql: r#"SELECT 1 AS "a\"b", __cursor_end() FROM t"#,
            expected_sql: r#"SELECT 1 AS "a\"b", __sqlbuild.cursor_end() FROM t"#,
        },
        NativeStubTestCase {
            dialect: "snowflake",
            description: "a cursor stub's text inside a Snowflake // comment is left as written",
            sql: "SELECT 1 FROM t // __cursor_end()\n",
            expected_sql: "SELECT 1 FROM t // __cursor_end()\n",
        },
        NativeStubTestCase {
            dialect: "snowflake",
            description: "a table function after a comma LATERAL stands as its stub relation alone",
            sql: "SELECT l.a FROM b, LATERAL __table_fn(\"f\")(b.id) AS l",
            expected_sql: "SELECT l.a FROM b, __sqlbuild_table_function_f AS l",
        },
        NativeStubTestCase {
            dialect: "snowflake",
            description: "a table function after JOIN lateral, in any case, stands as its stub relation alone",
            sql: "SELECT l.a FROM b JOIN lateral __table_fn(\"f\")(b.id) AS l ON TRUE",
            expected_sql: "SELECT l.a FROM b JOIN __sqlbuild_table_function_f AS l ON TRUE",
        },
        NativeStubTestCase {
            dialect: "snowflake",
            description: "a word ending in lateral before a table function is kept",
            sql: "SELECT l.a FROM collateral, __table_fn(\"f\")(1) AS l",
            expected_sql: "SELECT l.a FROM collateral, __sqlbuild_table_function_f AS l",
        },
        NativeStubTestCase {
            dialect: "snowflake",
            description: "lateral at the end of a comment before a table function is kept",
            sql: "SELECT l.a FROM b, -- not lateral\n__table_fn(\"f\")(1) AS l",
            expected_sql: "SELECT l.a FROM b, -- not lateral\n__sqlbuild_table_function_f AS l",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            normalize_native_sql(test_case.sql, test_case.dialect, "__sqlbuild.").as_deref(),
            Ok(test_case.expected_sql),
            "{}",
            test_case.description
        );
    }
}

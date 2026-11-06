//! Executed SQL is classified conservatively by its effect on cached relation metadata.

use crate::ddl_effects::main::qualified_relation_name_key::qualified_relation_name_key;
use crate::ddl_effects::main::relation_name_key::relation_name_key;
use crate::ddl_effects::main::statement_metadata_effect::statement_metadata_effect;
use crate::ddl_effects::models::StatementMetadataEffect;
use crate::ddl_effects::tests::test_types::{NameKeyTestCase, StatementEffectTestCase};

#[test]
fn given_executed_statement_when_classifying_then_reports_possible_metadata_changes() {
    let test_cases = [
        StatementEffectTestCase {
            description: "create or replace transient table",
            sql: "CREATE OR REPLACE TRANSIENT TABLE analytics.marts.orders__delta AS SELECT 1 AS id",
            expected_relation_names: &["orders__delta"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "quoted three-part view",
            sql: r#"CREATE OR REPLACE VIEW "ANALYTICS"."MARTS"."ORDERS_V" AS SELECT 1 AS id"#,
            expected_relation_names: &["orders_v"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "rename names both relations",
            sql: "ALTER TABLE marts.orders__stage RENAME TO marts.orders",
            expected_relation_names: &["orders", "orders__stage"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "backtick qualified name keys its last segment",
            sql: "CREATE OR REPLACE TABLE `project.dataset.Orders` AS SELECT 1 AS id",
            expected_relation_names: &["orders"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "bracket qualified name",
            sql: "DROP TABLE IF EXISTS [dbo].[orders__delta]",
            expected_relation_names: &["orders__delta"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "select into after a common table expression",
            sql: "WITH v AS (SELECT 1 AS id) SELECT id INTO dbo.orders_history FROM v",
            expected_relation_names: &["orders_history"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "copy into a table",
            sql: "COPY INTO raw.orders FROM @landing/orders",
            expected_relation_names: &["orders"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "separator inside a literal is data",
            sql: "CREATE OR REPLACE TABLE marts.notes AS SELECT 'a;b -- c' AS note",
            expected_relation_names: &["notes"],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "function body in dollar quotes",
            sql: "CREATE OR REPLACE FUNCTION f() RETURNS INT AS $$ SELECT 1; SELECT 2 $$",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "create schema if missing",
            sql: "CREATE SCHEMA IF NOT EXISTS analytics.marts",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "procedure call",
            sql: "CALL marts.rebuild_orders()",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "multi statement",
            sql: "INSERT INTO t VALUES (1); DROP TABLE t",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "unterminated quoted name",
            sql: r#"CREATE TABLE "open AS SELECT 1"#,
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "unquoted non-ascii name",
            sql: "CREATE TABLE s.café (id INT)",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "backslash-escaped quote is ambiguous",
            sql: r"SELECT 'a\' AS x FROM t WHERE y = '; DROP TABLE u'",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "rename target followed by more text",
            sql: "ALTER TABLE a RENAME TO b c",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "query tag for the session",
            sql: "ALTER SESSION SET QUERY_TAG = 'orders'",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "session setting reset",
            sql: "ALTER SESSION UNSET QUERY_TAG",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "warehouse for the session",
            sql: "USE WAREHOUSE transform_wh",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "search path changes how unqualified names resolve",
            sql: "ALTER SESSION SET SEARCH_PATH = 'marts, public'",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "identifier case setting changes how names resolve",
            sql: "ALTER SESSION SET QUOTED_IDENTIFIERS_IGNORE_CASE = TRUE",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "query tag followed by another parameter",
            sql: "ALTER SESSION SET QUERY_TAG = 'orders' SEARCH_PATH = 'marts'",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "query tag with an escaped quote",
            sql: "ALTER SESSION SET QUERY_TAG = 'order''s run'",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "statement timeout listed before an identifier case setting",
            sql: "ALTER SESSION SET STATEMENT_TIMEOUT_IN_SECONDS=10,QUOTED_IDENTIFIERS_IGNORE_CASE=TRUE",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "cached result setting listed before an identifier case setting",
            sql: "ALTER SESSION SET USE_CACHED_RESULT=FALSE,QUOTED_IDENTIFIERS_IGNORE_CASE=TRUE",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "statement timeout without spaces around the equals sign",
            sql: "ALTER SESSION SET STATEMENT_TIMEOUT_IN_SECONDS=10",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "statement timeout for the session",
            sql: "ALTER SESSION SET STATEMENT_TIMEOUT_IN_SECONDS = 3600",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "duckdb search path changes how unqualified names resolve",
            sql: "SET search_path = 'staging,main'",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "scoped search path in another case changes unqualified names",
            sql: "set session SEARCH_PATH to staging",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "duckdb default schema changes unqualified names",
            sql: "SET schema = 'staging'",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "reset search path changes unqualified names",
            sql: "RESET search_path",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "other settings and session variables leave names alone",
            sql: "SET threads = 4",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "a session variable named like a schema setting prefix is neutral",
            sql: "SET schema_owner = 'orders'",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "schema for the session changes unqualified names",
            sql: "USE SCHEMA analytics.marts",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "database for the session changes unqualified names",
            sql: "USE DATABASE analytics",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "role for the session changes visible relations",
            sql: "USE ROLE transformer",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "bare use names a database",
            sql: "USE analytics",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "dynamic sql is ambiguous",
            sql: "EXECUTE IMMEDIATE 'DROP TABLE orders'",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "scripting block is ambiguous",
            sql: "BEGIN DROP TABLE orders; END",
            expected_relation_names: &[],
            expected_invalidates_all: true,
            expected_ends_transaction: false,
        },
        StatementEffectTestCase {
            description: "commit ends the transaction",
            sql: "COMMIT;",
            expected_relation_names: &[],
            expected_invalidates_all: false,
            expected_ends_transaction: true,
        },
    ];
    for test_case in test_cases {
        let effect: StatementMetadataEffect = statement_metadata_effect(test_case.sql);
        assert_eq!(
            effect
                .relation_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<&str>>(),
            test_case.expected_relation_names,
            "{}",
            test_case.description
        );
        assert_eq!(
            effect.invalidates_all, test_case.expected_invalidates_all,
            "{}",
            test_case.description
        );
        assert_eq!(
            effect.ends_transaction, test_case.expected_ends_transaction,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_qualified_name_when_keying_then_uses_last_part_or_reports_unparseable() {
    let test_cases = [
        NameKeyTestCase {
            description: "unquoted part starting with a digit",
            name: "my-proj.ds.2024_orders",
            expected_key: Some("2024_orders"),
        },
        NameKeyTestCase {
            description: "backticked path keys its last segment",
            name: "`my-proj.ds.Orders`",
            expected_key: Some("orders"),
        },
        NameKeyTestCase {
            description: "quoted dotted part keeps its dot",
            name: r#""ANALYTICS"."MARTS"."my.table""#,
            expected_key: Some("my.table"),
        },
        NameKeyTestCase {
            description: "trailing text is unparseable",
            name: "ds.orders extra",
            expected_key: None,
        },
        NameKeyTestCase {
            description: "unquoted non-ascii part is unparseable",
            name: "ds.café",
            expected_key: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            qualified_relation_name_key(test_case.name).as_deref(),
            test_case.expected_key,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_relation_name_when_keying_then_strips_quotes_and_casefolds() {
    let test_cases = [
        NameKeyTestCase {
            description: "doubled quotes unescape",
            name: r#""Or""ders""#,
            expected_key: Some(r#"or"ders"#),
        },
        NameKeyTestCase {
            description: "python casefold expands sharp s",
            name: "STRAßE",
            expected_key: Some("strasse"),
        },
        NameKeyTestCase {
            description: "bracketed name",
            name: "[Orders]",
            expected_key: Some("orders"),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            Some(relation_name_key(test_case.name).as_str()),
            test_case.expected_key,
            "{}",
            test_case.description
        );
    }
}

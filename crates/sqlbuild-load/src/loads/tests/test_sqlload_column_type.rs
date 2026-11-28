use sqlload::column_types::models::Warehouse;

use crate::loads::_helpers::column_types::sqlload_column_type;
use crate::loads::tests::helpers::{advice_creates_exactly, logical_types};
use crate::loads::tests::test_types::{AdviceRoundTripTestCase, SqlloadColumnTypeTestCase};

#[test]
fn given_a_declared_type_when_handing_it_to_sqlload_then_only_a_warehouse_type_it_cannot_read_is_pinned()
 {
    let test_cases = [
        SqlloadColumnTypeTestCase {
            description: "a logical type SQLLoad reads is passed as written",
            warehouse: Warehouse::DuckDb,
            declared: "BIGINT",
            expected_sqlload_type: "BIGINT",
        },
        SqlloadColumnTypeTestCase {
            description: "a DuckDB type SQLLoad has no logical type for is pinned",
            warehouse: Warehouse::DuckDb,
            declared: "HUGEINT",
            expected_sqlload_type: r#"native("HUGEINT")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a DuckDB array is pinned",
            warehouse: Warehouse::DuckDb,
            declared: "BIGINT[]",
            expected_sqlload_type: r#"native("BIGINT[]")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a Postgres type SQLLoad has no logical type for is pinned",
            warehouse: Warehouse::Postgres,
            declared: "inet",
            expected_sqlload_type: r#"native("inet")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a ClickHouse nullable type is pinned",
            warehouse: Warehouse::ClickHouse,
            declared: "Nullable(Int64)",
            expected_sqlload_type: r#"native("Nullable(Int64)")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "ClickHouse's own TIMESTAMP, which SQLLoad refuses, is pinned as written",
            warehouse: Warehouse::ClickHouse,
            declared: "TIMESTAMP",
            expected_sqlload_type: r#"native("TIMESTAMP")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "Snowflake's TIMESTAMP, which follows the account's mapping, is pinned",
            warehouse: Warehouse::Snowflake,
            declared: "TIMESTAMP",
            expected_sqlload_type: r#"native("TIMESTAMP")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a Snowflake type SQLLoad reads is passed as written",
            warehouse: Warehouse::Snowflake,
            declared: "NUMBER(38,0)",
            expected_sqlload_type: "NUMBER(38,0)",
        },
        SqlloadColumnTypeTestCase {
            description: "BigQuery's BIGNUMERIC, wider than SQLLoad's decimal, is pinned",
            warehouse: Warehouse::BigQuery,
            declared: "BIGNUMERIC",
            expected_sqlload_type: r#"native("BIGNUMERIC")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a Databricks map is pinned",
            warehouse: Warehouse::Databricks,
            declared: "MAP<STRING, BIGINT>",
            expected_sqlload_type: r#"native("MAP<STRING, BIGINT>")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a name no warehouse knows is left to SQLLoad's own error",
            warehouse: Warehouse::Postgres,
            declared: "BIGNT",
            expected_sqlload_type: "BIGNT",
        },
        SqlloadColumnTypeTestCase {
            description: "an explicit pin is passed as written",
            warehouse: Warehouse::ClickHouse,
            declared: r#"native("Int32")"#,
            expected_sqlload_type: r#"native("Int32")"#,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            sqlload_column_type(test_case.declared, test_case.warehouse),
            test_case.expected_sqlload_type,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_any_type_sqlload_creates_when_advising_it_then_declaring_the_advice_creates_it_exactly() {
    let test_cases = [
        AdviceRoundTripTestCase {
            description: "DuckDB advice, its arrays included",
            warehouse: Warehouse::DuckDb,
            expected_advice_creates_exactly: true,
        },
        AdviceRoundTripTestCase {
            description: "Postgres advice, its arrays included",
            warehouse: Warehouse::Postgres,
            expected_advice_creates_exactly: true,
        },
        AdviceRoundTripTestCase {
            description: "ClickHouse advice, every type Nullable",
            warehouse: Warehouse::ClickHouse,
            expected_advice_creates_exactly: true,
        },
        AdviceRoundTripTestCase {
            description: "Snowflake advice, ARRAY read back as VARIANT included",
            warehouse: Warehouse::Snowflake,
            expected_advice_creates_exactly: true,
        },
        AdviceRoundTripTestCase {
            description: "BigQuery advice",
            warehouse: Warehouse::BigQuery,
            expected_advice_creates_exactly: true,
        },
        AdviceRoundTripTestCase {
            description: "Databricks advice",
            warehouse: Warehouse::Databricks,
            expected_advice_creates_exactly: true,
        },
    ];
    for test_case in test_cases {
        for column_type in &logical_types() {
            assert_eq!(
                advice_creates_exactly(test_case.warehouse, column_type),
                test_case.expected_advice_creates_exactly,
                "{}: {column_type}",
                test_case.description
            );
        }
    }
}

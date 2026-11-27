use sqlload::adapters::dispatch::main::dialect::sql_adapter::sql_adapter;
use sqlload::column_types::models::{ColumnType, Warehouse};

use crate::loads::main::declared_type_mismatch::{created_type, declared_type_mismatch};
use crate::loads::main::sqlload_column_type::sqlload_column_type;
use crate::loads::tests::test_types::SqlloadColumnTypeTestCase;

const WAREHOUSES: [Warehouse; 6] = [
    Warehouse::DuckDb,
    Warehouse::Postgres,
    Warehouse::ClickHouse,
    Warehouse::Snowflake,
    Warehouse::BigQuery,
    Warehouse::Databricks,
];

#[test]
fn given_a_declared_type_when_handing_it_to_sqlload_then_only_a_warehouse_type_it_cannot_read_is_pinned()
 {
    let test_cases = [
        SqlloadColumnTypeTestCase {
            description: "a logical type SQLLoad reads is passed as written",
            warehouse: Warehouse::DuckDb,
            declared: "BIGINT",
            expected: "BIGINT",
        },
        SqlloadColumnTypeTestCase {
            description: "a DuckDB type SQLLoad has no logical type for is pinned",
            warehouse: Warehouse::DuckDb,
            declared: "HUGEINT",
            expected: r#"native("HUGEINT")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a DuckDB array is pinned",
            warehouse: Warehouse::DuckDb,
            declared: "BIGINT[]",
            expected: r#"native("BIGINT[]")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a Postgres type SQLLoad has no logical type for is pinned",
            warehouse: Warehouse::Postgres,
            declared: "inet",
            expected: r#"native("inet")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a ClickHouse nullable type is pinned",
            warehouse: Warehouse::ClickHouse,
            declared: "Nullable(Int64)",
            expected: r#"native("Nullable(Int64)")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "ClickHouse's own TIMESTAMP, which SQLLoad refuses, is pinned as written",
            warehouse: Warehouse::ClickHouse,
            declared: "TIMESTAMP",
            expected: r#"native("TIMESTAMP")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "Snowflake's TIMESTAMP, which follows the account's mapping, is pinned",
            warehouse: Warehouse::Snowflake,
            declared: "TIMESTAMP",
            expected: r#"native("TIMESTAMP")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a Snowflake type SQLLoad reads is passed as written",
            warehouse: Warehouse::Snowflake,
            declared: "NUMBER(38,0)",
            expected: "NUMBER(38,0)",
        },
        SqlloadColumnTypeTestCase {
            description: "BigQuery's BIGNUMERIC, wider than SQLLoad's decimal, is pinned",
            warehouse: Warehouse::BigQuery,
            declared: "BIGNUMERIC",
            expected: r#"native("BIGNUMERIC")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a Databricks map is pinned",
            warehouse: Warehouse::Databricks,
            declared: "MAP<STRING, BIGINT>",
            expected: r#"native("MAP<STRING, BIGINT>")"#,
        },
        SqlloadColumnTypeTestCase {
            description: "a name no warehouse knows is left to SQLLoad's own error",
            warehouse: Warehouse::Postgres,
            declared: "BIGNT",
            expected: "BIGNT",
        },
        SqlloadColumnTypeTestCase {
            description: "an explicit pin is passed as written",
            warehouse: Warehouse::ClickHouse,
            declared: r#"native("Int32")"#,
            expected: r#"native("Int32")"#,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            sqlload_column_type(test_case.declared, test_case.warehouse),
            test_case.expected,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_any_type_sqlload_creates_when_advising_it_then_declaring_the_advice_creates_it_exactly() {
    let logical: [ColumnType; 15] = [
        ColumnType::Bool,
        ColumnType::SmallInt,
        ColumnType::Int,
        ColumnType::Decimal {
            precision: 18,
            scale: 2,
        },
        ColumnType::Float,
        ColumnType::Text,
        ColumnType::Varchar(20),
        ColumnType::Date,
        ColumnType::Time,
        ColumnType::TimestampTz,
        ColumnType::TimestampNtz,
        ColumnType::Json,
        ColumnType::Uuid,
        ColumnType::Binary,
        ColumnType::Array(Box::new(ColumnType::Int)),
    ];
    for warehouse in WAREHOUSES {
        for column_type in &logical {
            let created: String = sql_adapter(warehouse)
                .render_loader_logical_type(column_type)
                .sql;
            let advice: String = declared_type_mismatch(
                &[("c".to_owned(), column_type.to_string())],
                warehouse,
                |_, _| false,
            )
            .map(|mismatch| mismatch.declare)
            .unwrap_or_default();

            assert_eq!(
                created_type(&advice, warehouse).as_deref(),
                Some(created.as_str()),
                "{warehouse} {column_type}: advice {advice}"
            );
        }
    }
}

use crate::analysis_route::models::{DuckDbDialect, SnowflakeDialect, TypeMatch};
use crate::analysis_route::tests::test_types::NativeTypeMatchTestCase;
use crate::analysis_route::types::NativeDialect;

fn compared(same_type: bool) -> TypeMatch {
    TypeMatch::Compared {
        same_type,
        nullable_into_non_null: false,
    }
}

fn row(
    description: &'static str,
    declared: &'static str,
    inferred: &'static str,
    same_type: bool,
) -> NativeTypeMatchTestCase {
    NativeTypeMatchTestCase {
        description,
        declared,
        inferred: Some(inferred),
        expected_match: compared(same_type),
    }
}

#[test]
fn given_duckdb_contract_types_when_matching_then_the_types_duckdb_stores_are_compared() {
    let test_cases = [
        row(
            "TEXT(5) is VARCHAR, whose length DuckDB ignores",
            "VARCHAR(5)",
            "TEXT(5)",
            true,
        ),
        row(
            "an unsized VARCHAR meets a sized one",
            "VARCHAR(5)",
            "VARCHAR",
            true,
        ),
        row("STRING is VARCHAR", "VARCHAR", "STRING", true),
        row("BPCHAR is VARCHAR", "VARCHAR", "BPCHAR", true),
        row("INT4 is INTEGER", "INTEGER", "INT4", true),
        row("SIGNED is INTEGER", "INTEGER", "SIGNED", true),
        row("INT8 is BIGINT", "BIGINT", "INT8", true),
        row("LONG is BIGINT", "BIGINT", "LONG", true),
        row("INT2 is SMALLINT", "SMALLINT", "INT2", true),
        row("SHORT is SMALLINT", "SMALLINT", "SHORT", true),
        row("INT1 is TINYINT", "TINYINT", "INT1", true),
        row("REAL is FLOAT", "FLOAT", "REAL", true),
        row("FLOAT4 is FLOAT", "REAL", "FLOAT4", true),
        row("FLOAT8 is DOUBLE", "DOUBLE", "FLOAT8", true),
        row(
            "a bare DECIMAL is DECIMAL(18,3)",
            "DECIMAL",
            "DECIMAL(18,3)",
            true,
        ),
        row(
            "DECIMAL(10) is DECIMAL(10,0)",
            "DECIMAL(10,0)",
            "DECIMAL(10)",
            true,
        ),
        row("LOGICAL is BOOLEAN", "BOOLEAN", "LOGICAL", true),
        row("DATETIME is TIMESTAMP", "TIMESTAMP", "DATETIME", true),
        row("BYTEA is BLOB", "BLOB", "BYTEA", true),
        row(
            "a wider decimal is another type",
            "DECIMAL(12,2)",
            "DECIMAL(10,2)",
            false,
        ),
        row("BIGINT is not INTEGER", "BIGINT", "INTEGER", false),
        row("DOUBLE is not FLOAT", "DOUBLE", "FLOAT", false),
        row("INTEGER is not VARCHAR", "INTEGER", "VARCHAR", false),
        row("DuckDB has no NUMBER type", "NUMBER", "INTEGER", false),
    ];
    for test_case in test_cases {
        assert_eq!(
            DuckDbDialect.type_match(test_case.declared, test_case.inferred, false),
            test_case.expected_match,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_snowflake_contract_types_when_matching_then_the_types_snowflake_stores_are_compared() {
    let test_cases = [
        row("BYTEINT is NUMBER(38,0)", "NUMBER(38,0)", "BYTEINT", true),
        row("INT is NUMBER(38,0)", "NUMBER(38,0)", "INT", true),
        row(
            "NUMBER(10) is NUMBER(10,0)",
            "NUMBER(10,0)",
            "NUMBER(10)",
            true,
        ),
        row("DOUBLE is FLOAT", "FLOAT", "DOUBLE", true),
        row("FLOAT4 is FLOAT", "FLOAT", "FLOAT4", true),
        row("FLOAT8 is FLOAT", "FLOAT", "FLOAT8", true),
        row("REAL is DOUBLE", "DOUBLE", "REAL", true),
        row("CHAR(5) is VARCHAR(5)", "VARCHAR(5)", "CHAR(5)", true),
        row("a bare CHAR is VARCHAR(1)", "VARCHAR(1)", "CHAR", true),
        row("STRING is VARCHAR", "VARCHAR", "STRING", true),
        row(
            "DATETIME is TIMESTAMP_NTZ",
            "TIMESTAMP_NTZ",
            "DATETIME",
            true,
        ),
        row("TIME is TIME(9)", "TIME(9)", "TIME", true),
        row("TIME(3) is not TIME(9)", "TIME(9)", "TIME(3)", false),
        row(
            "a different scale is another type",
            "NUMBER(38,0)",
            "NUMBER(38,2)",
            false,
        ),
        row(
            "a sized VARCHAR is not another length",
            "VARCHAR(10)",
            "VARCHAR(5)",
            false,
        ),
        row(
            "an unsized VARCHAR is not VARCHAR(5)",
            "VARCHAR(5)",
            "VARCHAR",
            false,
        ),
        row("FLOAT is not NUMBER", "NUMBER(38,0)", "FLOAT", false),
        row("INTEGER is not VARCHAR", "INTEGER", "VARCHAR", false),
    ];
    for test_case in test_cases {
        assert_eq!(
            SnowflakeDialect.type_match(test_case.declared, test_case.inferred, false),
            test_case.expected_match,
            "{}",
            test_case.description
        );
    }
}

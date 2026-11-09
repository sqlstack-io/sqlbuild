use crate::analysis_route::models::{DuckDbDialect, SnowflakeDialect, TypeMatch};
use crate::analysis_route::tests::test_types::EngineTypeMatchTestCase;
use crate::analysis_route::types::NativeDialect;

#[test]
fn given_duckdb_contract_types_when_matching_then_the_types_duckdb_stores_are_compared() {
    let test_cases = [
        EngineTypeMatchTestCase {
            description: "TEXT(5) is VARCHAR, whose length DuckDB ignores",
            declared: "VARCHAR(5)",
            inferred: "TEXT(5)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "an unsized VARCHAR meets a sized one",
            declared: "VARCHAR(5)",
            inferred: "VARCHAR",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "STRING is VARCHAR",
            declared: "VARCHAR",
            inferred: "STRING",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "BPCHAR is VARCHAR",
            declared: "VARCHAR",
            inferred: "BPCHAR",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "INT4 is INTEGER",
            declared: "INTEGER",
            inferred: "INT4",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "SIGNED is INTEGER",
            declared: "INTEGER",
            inferred: "SIGNED",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "INT8 is BIGINT",
            declared: "BIGINT",
            inferred: "INT8",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "LONG is BIGINT",
            declared: "BIGINT",
            inferred: "LONG",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "INT2 is SMALLINT",
            declared: "SMALLINT",
            inferred: "INT2",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "SHORT is SMALLINT",
            declared: "SMALLINT",
            inferred: "SHORT",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "INT1 is TINYINT",
            declared: "TINYINT",
            inferred: "INT1",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "REAL is FLOAT",
            declared: "FLOAT",
            inferred: "REAL",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "FLOAT4 is FLOAT",
            declared: "REAL",
            inferred: "FLOAT4",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "FLOAT8 is DOUBLE",
            declared: "DOUBLE",
            inferred: "FLOAT8",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a bare DECIMAL is DECIMAL(18,3)",
            declared: "DECIMAL",
            inferred: "DECIMAL(18,3)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "DECIMAL(10) is DECIMAL(10,0)",
            declared: "DECIMAL(10,0)",
            inferred: "DECIMAL(10)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "LOGICAL is BOOLEAN",
            declared: "BOOLEAN",
            inferred: "LOGICAL",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "DATETIME is TIMESTAMP",
            declared: "TIMESTAMP",
            inferred: "DATETIME",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "BYTEA is BLOB",
            declared: "BLOB",
            inferred: "BYTEA",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a wider decimal is another type",
            declared: "DECIMAL(12,2)",
            inferred: "DECIMAL(10,2)",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "BIGINT is not INTEGER",
            declared: "BIGINT",
            inferred: "INTEGER",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "DOUBLE is not FLOAT",
            declared: "DOUBLE",
            inferred: "FLOAT",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "INTEGER is not VARCHAR",
            declared: "INTEGER",
            inferred: "VARCHAR",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "DuckDB has no NUMBER type",
            declared: "NUMBER",
            inferred: "INTEGER",
            expected_same_type: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            DuckDbDialect.type_match(test_case.declared, Some(test_case.inferred), false),
            TypeMatch::Compared {
                same_type: test_case.expected_same_type,
                nullable_into_non_null: false,
            },
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_snowflake_contract_types_when_matching_then_the_types_snowflake_stores_are_compared() {
    let test_cases = [
        EngineTypeMatchTestCase {
            description: "BYTEINT is NUMBER(38,0)",
            declared: "NUMBER(38,0)",
            inferred: "BYTEINT",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "INT is NUMBER(38,0)",
            declared: "NUMBER(38,0)",
            inferred: "INT",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "NUMBER(10) is NUMBER(10,0)",
            declared: "NUMBER(10,0)",
            inferred: "NUMBER(10)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "DOUBLE is FLOAT",
            declared: "FLOAT",
            inferred: "DOUBLE",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "FLOAT4 is FLOAT",
            declared: "FLOAT",
            inferred: "FLOAT4",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "FLOAT8 is FLOAT",
            declared: "FLOAT",
            inferred: "FLOAT8",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "REAL is DOUBLE",
            declared: "DOUBLE",
            inferred: "REAL",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "CHAR(5) is VARCHAR(5)",
            declared: "VARCHAR(5)",
            inferred: "CHAR(5)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a bare CHAR is VARCHAR(1)",
            declared: "VARCHAR(1)",
            inferred: "CHAR",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "STRING is VARCHAR",
            declared: "VARCHAR",
            inferred: "STRING",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "DATETIME is TIMESTAMP_NTZ",
            declared: "TIMESTAMP_NTZ",
            inferred: "DATETIME",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "TIME is TIME(9)",
            declared: "TIME(9)",
            inferred: "TIME",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "TIME(3) is not TIME(9)",
            declared: "TIME(9)",
            inferred: "TIME(3)",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "a different scale is another type",
            declared: "NUMBER(38,0)",
            inferred: "NUMBER(38,2)",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "a sized VARCHAR is not another length",
            declared: "VARCHAR(10)",
            inferred: "VARCHAR(5)",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "an unsized VARCHAR is not VARCHAR(5)",
            declared: "VARCHAR(5)",
            inferred: "VARCHAR",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "FLOAT is not NUMBER",
            declared: "NUMBER(38,0)",
            inferred: "FLOAT",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "INTEGER is not VARCHAR",
            declared: "INTEGER",
            inferred: "VARCHAR",
            expected_same_type: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            SnowflakeDialect.type_match(test_case.declared, Some(test_case.inferred), false),
            TypeMatch::Compared {
                same_type: test_case.expected_same_type,
                nullable_into_non_null: false,
            },
            "{}",
            test_case.description
        );
    }
}

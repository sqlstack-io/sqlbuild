use crate::type_system::errors::TypeNormalizationError;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::TypeFamily;
use crate::type_system::tests::helpers::normalization_text;
use crate::type_system::tests::test_types::{BracketDepthTestCase, NormalizeTypeTestCase};

#[test]
fn given_type_strings_when_normalizing_then_python_normalization_is_returned() {
    let test_cases = [
        NormalizeTypeTestCase {
            description: "integer aliases keep their spelling outside Snowflake and BigQuery",
            type_sql: "integer",
            dialect: "duckdb",
            expected_normalization: Ok("INT | integer | - | - | - | -"),
        },
        NormalizeTypeTestCase {
            description: "BigQuery integers are INT64",
            type_sql: "INT",
            dialect: "bigquery",
            expected_normalization: Ok("INT64 | integer | - | - | - | -"),
        },
        NormalizeTypeTestCase {
            description: "Snowflake integers are DECIMAL(38,0)",
            type_sql: "BIGINT",
            dialect: "snowflake",
            expected_normalization: Ok("DECIMAL(38,0) | decimal | 38 | 0 | - | -"),
        },
        NormalizeTypeTestCase {
            description: "Snowflake NUMBER is a custom type rewritten to DECIMAL",
            type_sql: "NUMBER(12, 2)",
            dialect: "snowflake",
            expected_normalization: Ok("DECIMAL(12,2) | decimal | 12 | 2 | - | -"),
        },
        NormalizeTypeTestCase {
            description: "Snowflake NUMBER without precision takes the integer default",
            type_sql: "NUMBER",
            dialect: "snowflake",
            expected_normalization: Ok("DECIMAL(38,0) | decimal | 38 | 0 | - | -"),
        },
        NormalizeTypeTestCase {
            description: "Snowflake unbounded text takes the default length",
            type_sql: "TEXT",
            dialect: "snowflake",
            expected_normalization: Ok("VARCHAR(16777216) | string | - | - | 16777216 | -"),
        },
        NormalizeTypeTestCase {
            description: "a sized TEXT is a VARCHAR of that length",
            type_sql: "TEXT(5)",
            dialect: "duckdb",
            expected_normalization: Ok("VARCHAR(5) | string | - | - | 5 | -"),
        },
        NormalizeTypeTestCase {
            description: "a sized STRING is a VARCHAR of that length on Snowflake",
            type_sql: "STRING(5)",
            dialect: "snowflake",
            expected_normalization: Ok("VARCHAR(5) | string | - | - | 5 | -"),
        },
        NormalizeTypeTestCase {
            description: "BigQuery spells every variable-length text type STRING",
            type_sql: "VARCHAR(5)",
            dialect: "bigquery",
            expected_normalization: Ok("STRING(5) | string | - | - | 5 | -"),
        },
        NormalizeTypeTestCase {
            description: "Snowflake timestamp aliases",
            type_sql: "TIMESTAMP_NTZ",
            dialect: "snowflake",
            expected_normalization: Ok("TIMESTAMP_NTZ | timestamp | - | - | - | -"),
        },
        NormalizeTypeTestCase {
            description: "decimal precision and scale",
            type_sql: "decimal(10, 2)",
            dialect: "duckdb",
            expected_normalization: Ok("DECIMAL(10,2) | decimal | 10 | 2 | - | -"),
        },
        NormalizeTypeTestCase {
            description: "nested types are other",
            type_sql: "ARRAY<STRUCT<a INT>>",
            dialect: "bigquery",
            expected_normalization: Ok("ARRAY<STRUCT<AINT64>> | other | - | - | - | -"),
        },
        NormalizeTypeTestCase {
            description: "a parse failure falls back to the text and reports the error",
            type_sql: "order status",
            dialect: "generic",
            expected_normalization: Ok(
                "ORDERSTATUS | other | - | - | - | Parse error at line 1, column 13: Unexpected token after data type: status",
            ),
        },
        NormalizeTypeTestCase {
            description: "every Polyglot dialect is normalized natively",
            type_sql: "INT",
            dialect: "mysql",
            expected_normalization: Ok("INT | integer | - | - | - | -"),
        },
        NormalizeTypeTestCase {
            description: "a dialect Polyglot does not know raises Python's error",
            type_sql: "INT",
            dialect: "motherduck",
            expected_normalization: Err("Unknown dialect: motherduck"),
        },
        NormalizeTypeTestCase {
            description: "non-ASCII text upper-cases with Python's Unicode mapping",
            type_sql: "stra\u{df}e",
            dialect: "generic",
            expected_normalization: Ok("STRASSE | other | - | - | - | -"),
        },
        NormalizeTypeTestCase {
            description: "a parameter beyond i64 keeps Python's integer",
            type_sql: "DECIMAL(99999999999999999999, 2)",
            dialect: "duckdb",
            expected_normalization: Ok(
                "DECIMAL(99999999999999999999,2) | decimal | 99999999999999999999 | 2 | - | \
                 Parse error at line 1, column 30: Invalid number: 99999999999999999999",
            ),
        },
        NormalizeTypeTestCase {
            description: "Unicode decimal digits are Python integers",
            type_sql: "VARCHAR(\u{663})",
            dialect: "snowflake",
            expected_normalization: Ok(
                "VARCHAR(3) | string | - | - | 3 | Parse error at line 1, column 10: Expected number",
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            normalize_type(test_case.type_sql, test_case.dialect)
                .as_ref()
                .map(normalization_text)
                .map_err(TypeNormalizationError::message)
                .as_ref()
                .map(String::as_str)
                .map_err(String::as_str),
            test_case.expected_normalization,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_array_suffixes_when_normalizing_on_a_worker_stack_then_python_supported_depths_answer() {
    let test_cases = [
        BracketDepthTestCase {
            description: "at the old native bracket cap",
            depth: 32,
            expected_native: true,
        },
        BracketDepthTestCase {
            description: "one past the old native bracket cap",
            depth: 33,
            expected_native: true,
        },
        BracketDepthTestCase {
            description: "a depth the Python wheel also normalizes, within a debug-build worker stack",
            depth: 200,
            expected_native: true,
        },
    ];

    for test_case in test_cases {
        let type_sql: String = format!("INT{}", "[]".repeat(test_case.depth));
        let expected_name: String = type_sql.clone();
        let answered: bool = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                normalize_type(&type_sql, "duckdb").is_ok_and(|normalization| {
                    normalization.normalized.normalized_name == expected_name
                })
            })
            .expect("the test thread starts")
            .join()
            .expect("normalization does not overflow the stack");
        assert_eq!(
            answered, test_case.expected_native,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_complex_types_holding_a_timestamp_when_normalizing_text_then_the_top_level_type_decides() {
    let test_cases = [
        (
            "an array of timestamps is an array",
            "ARRAY<TIMESTAMP>",
            TypeFamily::Other,
        ),
        (
            "a map of structs holding a timestamp is a map",
            "MAP<STRING,STRUCT<a:INT,b:TIMESTAMP_NTZ>>",
            TypeFamily::Other,
        ),
        (
            "a struct holding a timestamp is a struct",
            "STRUCT<t TIMESTAMP>",
            TypeFamily::Other,
        ),
        (
            "a bare timestamp is a timestamp",
            "TIMESTAMP",
            TypeFamily::Timestamp,
        ),
        (
            "a timestamp with precision is a timestamp",
            "TIMESTAMP(3)",
            TypeFamily::Timestamp,
        ),
        (
            "a zoned timestamp is a timestamp",
            "TIMESTAMP WITH TIME ZONE",
            TypeFamily::Timestamp,
        ),
        (
            "Snowflake's NTZ alias is a timestamp",
            "TIMESTAMP_NTZ",
            TypeFamily::Timestamp,
        ),
    ];
    for (description, type_sql, expected_family) in test_cases {
        assert_eq!(
            normalize_text_type(type_sql, None).family,
            expected_family,
            "{description}"
        );
    }
}

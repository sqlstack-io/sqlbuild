use crate::analysis_route::main::same_column_type::same_column_type;
use crate::analysis_route::tests::test_types::EngineTypeMatchTestCase;

#[test]
fn given_declared_and_snowflake_listed_types_when_comparing_then_default_spellings_match() {
    let test_cases = [
        EngineTypeMatchTestCase {
            description: "an unsized VARCHAR is VARCHAR(16777216)",
            declared: "VARCHAR",
            inferred: "VARCHAR(16777216)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "TEXT and STRING are VARCHAR(16777216)",
            declared: "TEXT",
            inferred: "VARCHAR(16777216)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "STRING is VARCHAR(16777216)",
            declared: "STRING",
            inferred: "VARCHAR(16777216)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a bare NUMBER is NUMBER(38,0)",
            declared: "NUMBER",
            inferred: "NUMBER(38,0)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a bare DECIMAL is NUMBER(38,0)",
            declared: "DECIMAL",
            inferred: "NUMBER(38,0)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "INT is NUMBER(38,0)",
            declared: "INT",
            inferred: "NUMBER(38,0)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "BIGINT is NUMBER(38,0)",
            declared: "BIGINT",
            inferred: "NUMBER(38,0)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "NUMBER(18) is NUMBER(18,0)",
            declared: "NUMBER(18)",
            inferred: "NUMBER(18,0)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "DOUBLE is FLOAT",
            declared: "DOUBLE",
            inferred: "FLOAT",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a stated timestamp precision meets the listed unstated one",
            declared: "TIMESTAMP_NTZ(9)",
            inferred: "TIMESTAMP_NTZ",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "spelling case does not matter",
            declared: "varchar",
            inferred: "VARCHAR(16777216)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a shorter VARCHAR is a different type",
            declared: "VARCHAR(10)",
            inferred: "VARCHAR(16777216)",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "a scaled NUMBER is not an integer",
            declared: "NUMBER(18,2)",
            inferred: "NUMBER(38,0)",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "a string is not a number",
            declared: "VARCHAR",
            inferred: "NUMBER(38,0)",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "TIMESTAMP_TZ is not TIMESTAMP_NTZ",
            declared: "TIMESTAMP_TZ",
            inferred: "TIMESTAMP_NTZ",
            expected_same_type: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            same_column_type(Some("snowflake"), test_case.declared, test_case.inferred),
            test_case.expected_same_type,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_dialect_without_a_native_module_when_comparing_then_only_equal_text_matches() {
    let test_cases = [
        EngineTypeMatchTestCase {
            description: "equal text matches",
            declared: "VARCHAR",
            inferred: "varchar",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "a default length is not assumed",
            declared: "VARCHAR",
            inferred: "VARCHAR(16777216)",
            expected_same_type: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            same_column_type(None, test_case.declared, test_case.inferred),
            test_case.expected_same_type,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_declared_and_clickhouse_listed_types_when_comparing_then_aliases_match_what_clickhouse_creates()
 {
    let test_cases = [
        EngineTypeMatchTestCase {
            description: "BIGINT is the Nullable(Int64) ClickHouse creates for it",
            declared: "BIGINT",
            inferred: "Nullable(Int64)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "TEXT is Nullable(String)",
            declared: "TEXT",
            inferred: "Nullable(String)",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "DateTime64(6) is Nullable(DateTime64(6))",
            declared: "DateTime64(6)",
            inferred: "Nullable(DateTime64(6))",
            expected_same_type: true,
        },
        EngineTypeMatchTestCase {
            description: "TIMESTAMP is ClickHouse's DateTime, which holds whole seconds",
            declared: "TIMESTAMP",
            inferred: "Nullable(DateTime64(6))",
            expected_same_type: false,
        },
        EngineTypeMatchTestCase {
            description: "INTEGER is Int32, not Int64",
            declared: "INTEGER",
            inferred: "Nullable(Int64)",
            expected_same_type: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            same_column_type(Some("clickhouse"), test_case.declared, test_case.inferred),
            test_case.expected_same_type,
            "{}",
            test_case.description
        );
    }
}

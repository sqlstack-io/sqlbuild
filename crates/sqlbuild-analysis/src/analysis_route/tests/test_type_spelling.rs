use crate::analysis_route::models::{DuckDbDialect, SnowflakeDialect, SqliteDialect};
use crate::analysis_route::tests::test_types::TypeSpellingTestCase;
use crate::type_system::models::{NormalizedType, TypeFamily};

#[test]
fn given_a_type_spelling_when_a_dialect_normalises_it_then_it_is_polyglots_type_or_none() {
    let test_cases = [
        TypeSpellingTestCase {
            description: "a DuckDB integer keeps Polyglot's INT",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "INTEGER",
            expected_type: Some(("INT", TypeFamily::Integer)),
        },
        TypeSpellingTestCase {
            description: "BLOB and VARBINARY swap names as Polyglot swaps them",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "BLOB",
            expected_type: Some(("VARBINARY", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "VARBINARY becomes BLOB",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "VARBINARY",
            expected_type: Some(("BLOB", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a sized VARCHAR keeps its length as a whole type",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "VARCHAR(10)",
            expected_type: Some(("VARCHAR(10)", TypeFamily::String)),
        },
        TypeSpellingTestCase {
            description: "a list element is renamed with its length",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "VARCHAR(10)[]",
            expected_type: Some(("TEXT(10)[]", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a field named like a type keeps its name",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "STRUCT(integer INTEGER)",
            expected_type: Some(("STRUCT(INTEGERINT)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a nested CHAR loses its length",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "STRUCT(a CHAR(3))",
            expected_type: Some(("STRUCT(ATEXT)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a whole CHAR(n) has no comparable name and stays unknown",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "CHAR(3)",
            expected_type: None,
        },
        TypeSpellingTestCase {
            description: "LIST(...) is compared as written",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "LIST(INTEGER)",
            expected_type: Some(("LIST(INTEGER)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a bare ROW keeps its name",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "ROW",
            expected_type: Some(("ROW", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "ROW with fields is a STRUCT",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "ROW(a INTEGER)",
            expected_type: Some(("STRUCT(AINT)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "DATETIME is renamed only inside another type",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "STRUCT(a datetime)",
            expected_type: Some(("STRUCT(ATIMESTAMP)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a whole DATETIME keeps its name",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "datetime",
            expected_type: Some(("DATETIME", TypeFamily::Datetime)),
        },
        TypeSpellingTestCase {
            description: "a timestamp list, which the fallback would call a timestamp, stays unknown",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "TIMESTAMP[]",
            expected_type: None,
        },
        TypeSpellingTestCase {
            description: "a struct holding a timestamp is not a timestamp",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "STRUCT(a TIMESTAMP, b VARCHAR)",
            expected_type: Some(("STRUCT(ATIMESTAMP,BTEXT)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a map key keeps its renamed length",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "MAP(VARCHAR(5), INTEGER)",
            expected_type: Some(("MAP(TEXT(5),INT)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a multi-word name is one alias",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "TIMESTAMP WITH TIME ZONE",
            expected_type: Some(("TIMESTAMPTZ", TypeFamily::Timestamp)),
        },
        TypeSpellingTestCase {
            description: "spaced decimal parameters are kept",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "DECIMAL(12, 2)",
            expected_type: Some(("DECIMAL(12,2)", TypeFamily::Decimal)),
        },
        TypeSpellingTestCase {
            description: "text after the type is not a type",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "VARCHAR NOT NULL",
            expected_type: None,
        },
        TypeSpellingTestCase {
            description: "REAL is FLOAT",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "REAL",
            expected_type: Some(("FLOAT", TypeFamily::Float)),
        },
        TypeSpellingTestCase {
            description: "an integer is NUMBER(38,0)",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "INTEGER",
            expected_type: Some(("DECIMAL(38,0)", TypeFamily::Decimal)),
        },
        TypeSpellingTestCase {
            description: "a nested INTEGER is INT",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "OBJECT(a INTEGER)",
            expected_type: Some(("OBJECT(AINT)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a nested TIMESTAMP_NTZ loses its underscore",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "OBJECT(a TIMESTAMP_NTZ)",
            expected_type: Some(("OBJECT(ATIMESTAMPNTZ)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "TIMESTAMP_NTZ drops its precision",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "TIMESTAMP_NTZ(9)",
            expected_type: Some(("TIMESTAMP_NTZ", TypeFamily::Timestamp)),
        },
        TypeSpellingTestCase {
            description: "TIMESTAMP(9), which Polyglot keeps sized, stays unknown",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "TIMESTAMP(9)",
            expected_type: None,
        },
        TypeSpellingTestCase {
            description: "array brackets are not Snowflake type syntax",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "INTEGER[]",
            expected_type: None,
        },
        TypeSpellingTestCase {
            description: "TEXT is the longest VARCHAR",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "TEXT",
            expected_type: Some(("VARCHAR(16777216)", TypeFamily::String)),
        },
        TypeSpellingTestCase {
            description: "a nested TEXT is VARCHAR",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "ARRAY(TEXT)",
            expected_type: Some(("ARRAY(VARCHAR)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "CHARACTER VARYING is VARCHAR with its length",
            dialect: "snowflake",
            normalize: |spelling| SnowflakeDialect.normalize_type(spelling),
            spelling: "CHARACTER VARYING(20)",
            expected_type: Some(("VARCHAR(20)", TypeFamily::String)),
        },
        TypeSpellingTestCase {
            description: "DOUBLE is REAL",
            dialect: "sqlite",
            normalize: |spelling| SqliteDialect.normalize_type(spelling),
            spelling: "DOUBLE",
            expected_type: Some(("REAL", TypeFamily::Float)),
        },
        TypeSpellingTestCase {
            description: "INT is INTEGER",
            dialect: "sqlite",
            normalize: |spelling| SqliteDialect.normalize_type(spelling),
            spelling: "INT",
            expected_type: Some(("INTEGER", TypeFamily::Integer)),
        },
        TypeSpellingTestCase {
            description: "a sized VARCHAR keeps its length",
            dialect: "sqlite",
            normalize: |spelling| SqliteDialect.normalize_type(spelling),
            spelling: "VARCHAR(255)",
            expected_type: Some(("VARCHAR(255)", TypeFamily::String)),
        },
        TypeSpellingTestCase {
            description: "INT(11), which Polyglot keeps sized, stays unknown",
            dialect: "sqlite",
            normalize: |spelling| SqliteDialect.normalize_type(spelling),
            spelling: "INT(11)",
            expected_type: None,
        },
        TypeSpellingTestCase {
            description: "a type name outside ASCII is one word, upper-cased as Python does",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "téxt",
            expected_type: Some(("T\u{c9}XT", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "a bare struct field outside ASCII is quoted, as Polyglot quotes it",
            dialect: "duckdb",
            normalize: |spelling| DuckDbDialect.normalize_type(spelling),
            spelling: "STRUCT(größe INTEGER)",
            expected_type: Some(("STRUCT(\"GRÖSSE\"INT)", TypeFamily::Other)),
        },
        TypeSpellingTestCase {
            description: "NUMERIC is DECIMAL",
            dialect: "sqlite",
            normalize: |spelling| SqliteDialect.normalize_type(spelling),
            spelling: "NUMERIC(10,2)",
            expected_type: Some(("DECIMAL(10,2)", TypeFamily::Decimal)),
        },
    ];
    for test_case in test_cases {
        let found: Option<(String, TypeFamily)> = (test_case.normalize)(test_case.spelling)
            .map(|normalized: NormalizedType| (normalized.normalized_name, normalized.family));
        assert_eq!(
            found,
            test_case
                .expected_type
                .map(|(name, family)| (name.to_owned(), family)),
            "{}: {}",
            test_case.dialect,
            test_case.description
        );
    }
}

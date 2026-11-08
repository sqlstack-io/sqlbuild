use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::models::{DatabricksDialect, LocatedGap, TypeMatch};
use crate::analysis_route::tests::test_types::{
    DatabricksTypeTestCase, NativeGapTestCase, NativeTypeMatchTestCase, ParseFailureTestCase,
};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::{NormalizedType, TypeFamily};

#[test]
fn given_databricks_model_sql_when_probing_its_parser_then_only_sql_it_reads_whole_has_no_gap() {
    let test_cases = [
        NativeGapTestCase {
            description: "a query Databricks reads whole",
            sql: "SELECT 1 AS a FROM t WHERE region = @@@region",
            placeholders: &[("region", "'eu'")],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a UDF call's stub reads as one name",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "cursor bounds read as stub calls",
            sql: "SELECT * FROM t WHERE updated_at >= __cursor_start() AND updated_at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a syntax error is located where Databricks locates it",
            sql: "SELECT 1 FROM t WHERE )",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "Syntax error at or near ')'".to_owned(),
                message: "Syntax error at or near ')'".to_owned(),
                line: 1,
                column: 23,
                syntax: true,
            }),
        },
        NativeGapTestCase {
            description: "a construct the shared tree lacks is a gap named by the parser",
            sql: "VACUUM t",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "vacuum".to_owned(),
                message: "vacuum".to_owned(),
                line: 1,
                column: 1,
                syntax: false,
            }),
        },
        NativeGapTestCase {
            description: "a second statement is not read whole",
            sql: "SELECT 1;\nSELECT 2",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "more than one statement".to_owned(),
                message: "more than one statement".to_owned(),
                line: 1,
                column: 1,
                syntax: false,
            }),
        },
    ];
    for test_case in test_cases {
        let check = SyntaxCheck {
            sql: test_case.sql.to_owned(),
            placeholders: test_case
                .placeholders
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        };
        assert_eq!(
            native_gap(&DatabricksDialect, &check),
            Ok(test_case.expected_gap),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_databricks_type_spelling_when_normalising_then_its_parser_decides_and_synonyms_meet() {
    let test_cases = [
        DatabricksTypeTestCase {
            description: "INTEGER is INT",
            spelling: "INTEGER",
            expected_type: Some(("INT", TypeFamily::Integer)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "REAL is FLOAT",
            spelling: "real",
            expected_type: Some(("FLOAT", TypeFamily::Float)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "NUMERIC with arguments is DECIMAL with its precision and scale",
            spelling: "NUMERIC(9, 3)",
            expected_type: Some(("DECIMAL(9,3)", TypeFamily::Decimal)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "a sized string keeps its length",
            spelling: "VARCHAR(10)",
            expected_type: Some(("VARCHAR(10)", TypeFamily::String)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "VARCHAR alone is STRING",
            spelling: "varchar",
            expected_type: Some(("STRING", TypeFamily::String)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "TIMESTAMP_NTZ is Polyglot's TIMESTAMPNTZ",
            spelling: "TIMESTAMP_NTZ",
            expected_type: Some(("TIMESTAMPNTZ", TypeFamily::Timestamp)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "a struct field with or without its colon is one type",
            spelling: "STRUCT<a INT>",
            expected_type: Some(("STRUCT<A:INT>", TypeFamily::Other)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "synonyms inside a map and a struct",
            spelling: "MAP<STRING, STRUCT<a: NUMERIC(5,2), b: INTEGER>>",
            expected_type: Some((
                "MAP<STRING,STRUCT<A:DECIMAL(5,2),B:INT>>",
                TypeFamily::Other,
            )),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "LONG is BIGINT, where Polyglot keeps LONG",
            spelling: "LONG",
            expected_type: Some(("BIGINT", TypeFamily::Integer)),
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "SHORT is SMALLINT, where Polyglot knows no SHORT",
            spelling: "SHORT",
            expected_type: Some(("SMALLINT", TypeFamily::Integer)),
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "TIMESTAMP_LTZ is TIMESTAMP, where Polyglot keeps TIMESTAMPLTZ",
            spelling: "TIMESTAMP_LTZ",
            expected_type: Some(("TIMESTAMP", TypeFamily::Timestamp)),
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "DECIMAL alone is DECIMAL(10,0)",
            spelling: "DECIMAL",
            expected_type: Some(("DECIMAL(10,0)", TypeFamily::Decimal)),
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "DECIMAL(p) has scale 0",
            spelling: "DEC(9)",
            expected_type: Some(("DECIMAL(9,0)", TypeFamily::Decimal)),
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "a struct field's constraint and comment are not its type",
            spelling: "STRUCT<a: INT NOT NULL COMMENT 'x'>",
            expected_type: Some(("STRUCT<A:INT>", TypeFamily::Other)),
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "a collation is not its type",
            spelling: "VARCHAR(10) COLLATE UTF8_BINARY",
            expected_type: Some(("VARCHAR(10)", TypeFamily::String)),
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "an interval keeps its fields",
            spelling: "INTERVAL YEAR TO MONTH",
            expected_type: Some(("INTERVALYEARTOMONTH", TypeFamily::Other)),
            expected_as_polyglot: true,
        },
        DatabricksTypeTestCase {
            description: "DOUBLE PRECISION is not a Databricks spelling",
            spelling: "DOUBLE PRECISION",
            expected_type: None,
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "TEXT is not a Databricks type",
            spelling: "TEXT",
            expected_type: None,
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "BOOL is not a Databricks type",
            spelling: "BOOL",
            expected_type: None,
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "INT64 is not a Databricks type",
            spelling: "INT64",
            expected_type: None,
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "an array is ARRAY<T>, not T[]",
            spelling: "INTEGER[]",
            expected_type: None,
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "a map is MAP<K, V>, not MAP(K, V)",
            spelling: "MAP(VARCHAR, INTEGER)",
            expected_type: None,
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "an empty type",
            spelling: "  ",
            expected_type: None,
            expected_as_polyglot: false,
        },
        DatabricksTypeTestCase {
            description: "text that closes the cast is not a type",
            spelling: "INT) FROM t WHERE (1",
            expected_type: None,
            expected_as_polyglot: false,
        },
    ];
    for test_case in test_cases {
        let native: Option<NormalizedType> = DatabricksDialect.normalize_type(test_case.spelling);
        let polyglot: Option<NormalizedType> = normalize_type(test_case.spelling, "databricks")
            .ok()
            .map(|normalization| normalization.normalized);
        assert_eq!(
            (
                native
                    .as_ref()
                    .map(|native| (native.normalized_name.as_str(), native.family)),
                native.is_some() && native == polyglot
            ),
            (test_case.expected_type, test_case.expected_as_polyglot),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_databricks_refuses_when_parsing_then_the_gap_says_whether_it_is_a_syntax_error() {
    let test_cases = [
        ParseFailureTestCase {
            description: "a syntax error, at its byte offset after a multi-byte character",
            sql: "SELECT 'é' FROM t WHERE )".to_owned(),
            expected_syntax: true,
            expected_offset: Some(25),
        },
        ParseFailureTestCase {
            description: "invalid syntax at the end of the input",
            sql: "SELECT (".to_owned(),
            expected_syntax: true,
            expected_offset: Some(8),
        },
        ParseFailureTestCase {
            description: "the nesting limit is not a syntax error",
            sql: format!("SELECT {}1{}", "(".repeat(300), ")".repeat(300)),
            expected_syntax: false,
            expected_offset: None,
        },
    ];
    for test_case in test_cases {
        let gap = DatabricksDialect
            .parse(&test_case.sql)
            .expect_err(test_case.description);
        assert_eq!(
            (gap.syntax, test_case.expected_offset.map(|_| gap.offset)),
            (test_case.expected_syntax, test_case.expected_offset),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_declared_and_an_inferred_databricks_type_when_matching_then_a_length_check_on_string_is_string()
 {
    let compared = |same_type: bool| TypeMatch::Compared {
        same_type,
        nullable_into_non_null: false,
    };
    let test_cases = [
        NativeTypeMatchTestCase {
            description: "a VARCHAR(n) column takes the STRING a cast to VARCHAR(n) gives",
            declared: "VARCHAR(19)",
            inferred: Some("STRING"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "a CHAR(n) column takes a STRING too",
            declared: "char(3)",
            inferred: Some("string"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "a STRING column takes a VARCHAR(n) column, which Databricks reads as STRING",
            declared: "STRING",
            inferred: Some("VARCHAR(19)"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "a VARCHAR(n) column does not take an integer",
            declared: "VARCHAR(19)",
            inferred: Some("INT"),
            expected_match: compared(false),
        },
        NativeTypeMatchTestCase {
            description: "synonyms match through the alias fold",
            declared: "INTEGER",
            inferred: Some("INT"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "sized strings of other lengths differ",
            declared: "VARCHAR(19)",
            inferred: Some("VARCHAR(20)"),
            expected_match: compared(false),
        },
        NativeTypeMatchTestCase {
            description: "a type Databricks refuses is not compared",
            declared: "TEXT",
            inferred: Some("STRING"),
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
        NativeTypeMatchTestCase {
            description: "an untyped output is not compared",
            declared: "VARCHAR(19)",
            inferred: None,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            DatabricksDialect.type_match(test_case.declared, test_case.inferred, false),
            test_case.expected_match,
            "{}",
            test_case.description
        );
    }
}
